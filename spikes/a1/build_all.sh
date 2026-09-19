#!/bin/sh
# builds every candidate combo into bins/<name>
set -e
mkdir -p bins
b() { name=$1; feats=$2; cargo build --release --features "$feats" -q 2>&1 | grep -E "^error" -A5 || true; cp target/release/a1 bins/$name; }
b reqwest-htmd http-reqwest,conv-htmd
b reqwest-html2md http-reqwest,conv-html2md
b reqwest-html2text http-reqwest,conv-html2text
b reqwest-none http-reqwest,conv-none
b hyper-htmd http-hyper,conv-htmd
b hyper-none http-hyper,conv-none
b ureq-htmd http-ureq,conv-htmd
b ureq-none http-ureq,conv-none
b reqwest-htmd-mimalloc http-reqwest,conv-htmd,mimalloc
b hyper-htmd-mimalloc http-hyper,conv-htmd,mimalloc
b ureq-htmd-mimalloc http-ureq,conv-htmd,mimalloc
