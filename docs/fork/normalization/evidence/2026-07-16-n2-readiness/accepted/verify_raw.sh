#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
while read -r expected path; do
  actual=$(gzip -dc "$path.gz" | shasum -a 256 | cut -d ' ' -f1)
  [[ "$actual" == "$expected" ]] || {
    echo "FAIL $path expected=$expected actual=$actual"
    exit 1
  }
  echo "$path: OK"
done < RAW_SHA256SUMS
