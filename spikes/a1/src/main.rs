//! A-1 spike: minimal rmcp stdio `fetch` server with swappable HTTP client / HTML->md backends.
use rmcp::{
    handler::server::wrapper::Parameters, schemars, tool, tool_router, ServiceExt,
};
use serde::Deserialize;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

const BODY_CAP: usize = 16 * 1024 * 1024;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FetchParams {
    /// URL to fetch
    url: String,
    /// Maximum number of characters to return (default 5000)
    max_length: Option<usize>,
    /// Character offset to start from (default 0)
    start_index: Option<usize>,
    /// Return raw body without HTML simplification
    raw: Option<bool>,
}

#[derive(Clone)]
struct Fetch;

#[tool_router(server_handler)]
impl Fetch {
    #[tool(description = "Fetch a URL and return its content as markdown")]
    async fn fetch(&self, Parameters(p): Parameters<FetchParams>) -> Result<String, String> {
        let start = p.start_index.unwrap_or(0);
        let max = p.max_length.unwrap_or(5000);
        #[cfg(feature = "conv-lolhtml")]
        if !p.raw.unwrap_or(false) {
            return stream_md(&p.url, start, max).await;
        }
        let body = get_body(&p.url).await?;
        let text = String::from_utf8(body).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
        let out = if p.raw.unwrap_or(false) { text } else { convert(text) };
        Ok(out.chars().skip(start).take(max).collect())
    }
}

/// Streaming path: network chunks -> lol_html (push tokenizer) -> markdown; stops once start+max chars produced.
#[cfg(feature = "conv-lolhtml")]
async fn stream_md(url: &str, start: usize, max: usize) -> Result<String, String> {
    use lol_html::send::{HtmlRewriter, Settings};
    use lol_html::{element, text};
    use std::sync::{Arc as Rc, Mutex as RefCell};
    let need = start + max;
    let out = Rc::new(RefCell::new(String::new()));
    let skip = Rc::new(RefCell::new(0usize));
    let (o1, o2, s1, s2) = (out.clone(), out.clone(), skip.clone(), skip.clone());
    let mut rw = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![
                element!("script, style", move |el| {
                    *s1.lock().unwrap() += 1;
                    let s = s1.clone();
                    let _ = el.on_end_tag(lol_html::end_tag!(move |_end| { *s.lock().unwrap() -= 1; Ok(()) }));
                    Ok(())
                }),
                element!("h1,h2,h3,h4,h5,h6,p,li,div,section,br", move |el| {
                    let pre = match el.tag_name().as_str() { "h1" => "\n\n# ", "h2" => "\n\n## ", "h3" => "\n\n### ", "li" => "\n- ", "br" => "\n", _ => "\n\n" };
                    o1.lock().unwrap().push_str(pre);
                    Ok(())
                }),
                text!("*", move |t| {
                    if *s2.lock().unwrap() == 0 { o2.lock().unwrap().push_str(t.as_str()); }
                    Ok(())
                }),
            ],
            ..Settings::new_send()
        },
        |_: &[u8]| {},
    );
    let client = reqwest::Client::builder().build().map_err(|e| e.to_string())?;
    let mut resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    let mut total = 0usize;
    while let Some(c) = resp.chunk().await.map_err(|e| e.to_string())? {
        total += c.len();
        rw.write(&c).map_err(|e| e.to_string())?;
        if out.lock().unwrap().len() >= need || total > BODY_CAP { break; }
    }
    let _ = rw.end();
    let r: String = out.lock().unwrap().chars().skip(start).take(max).collect();
    Ok(r)
}

fn convert(html: String) -> String {
    #[cfg(feature = "conv-htmd")]
    { return htmd::convert(&html).unwrap_or(html); }
    #[cfg(feature = "conv-html2md")]
    { return html2md::parse_html(&html); }
    #[cfg(feature = "conv-html2text")]
    { return html2text::from_read(html.as_bytes(), 100).unwrap_or(html); }
    #[cfg(not(any(feature = "conv-htmd", feature = "conv-html2md", feature = "conv-html2text")))]
    { html }
}

#[cfg(feature = "http-reqwest")]
async fn get_body(url: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::Client::builder().build().map_err(|e| e.to_string())?;
    let mut resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    while let Some(c) = resp.chunk().await.map_err(|e| e.to_string())? {
        buf.extend_from_slice(&c);
        if buf.len() > BODY_CAP { return Err("too large".into()); }
    }
    Ok(buf)
}

#[cfg(feature = "http-hyper")]
async fn get_body(url: &str) -> Result<Vec<u8>, String> {
    use http_body_util::{BodyExt, Empty};
    use hyper::body::Bytes;
    use hyper_util::{client::legacy::Client, rt::TokioExecutor};
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_webpki_roots().https_or_http().enable_http1().build();
    let client: Client<_, Empty<Bytes>> = Client::builder(TokioExecutor::new()).build(https);
    let uri: hyper::Uri = url.parse().map_err(|e: hyper::http::uri::InvalidUri| e.to_string())?;
    let mut resp = client.get(uri).await.map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    while let Some(fr) = resp.body_mut().frame().await {
        let fr = fr.map_err(|e| e.to_string())?;
        if let Some(d) = fr.data_ref() {
            buf.extend_from_slice(d);
            if buf.len() > BODY_CAP { return Err("too large".into()); }
        }
    }
    Ok(buf)
}

#[cfg(feature = "http-ureq")]
async fn get_body(url: &str) -> Result<Vec<u8>, String> {
    let url = url.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut resp = ureq::get(&url).call().map_err(|e| e.to_string())?;
        resp.body_mut().with_config().limit(BODY_CAP as u64).read_to_vec().map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(any(feature = "http-reqwest", feature = "http-hyper"))]
    { let _ = rustls::crypto::ring::default_provider().install_default(); }
    let svc = Fetch.serve(rmcp::transport::stdio()).await?;
    svc.waiting().await?;
    Ok(())
}
