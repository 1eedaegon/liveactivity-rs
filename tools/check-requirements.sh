#!/usr/bin/env bash
set -euo pipefail

requirements=$(sed -n 's/^## \(REQ-[A-Z]-[0-9][0-9][0-9]\).*/\1/p' docs/requirements.md)

for requirement in $requirements; do
  if ! grep -R -q "$requirement" crates .github; then
    echo "missing test or CI evidence for $requirement" >&2
    exit 1
  fi
done

echo "every documented requirement has test or CI evidence"
