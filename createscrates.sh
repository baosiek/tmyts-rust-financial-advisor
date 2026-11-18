#!/bin/bash
for c in core llm market-data embed news-rag portfolio warden memory tools agent observability; do
  cargo new --lib crates/advisor-$c
done
for b in api mcp ingest evals; do
  cargo new --bin bins/advisor-$b
done
