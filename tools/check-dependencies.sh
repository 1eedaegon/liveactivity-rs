#!/usr/bin/env bash
# The interface alone depends on serialization and error crates only: no UI framework, no
# native handle and no build integration can reach it unnoticed.
set -euo pipefail

allowed='^(liveactivity|serde|serde_core|serde_derive|serde_json|thiserror|thiserror-impl|itoa|memchr|zmij|proc-macro2|quote|syn|unicode-ident)$'
unexpected=$(cargo tree --locked -p liveactivity --no-default-features -e normal --prefix none \
  | awk '{print $1}' | sort -u | grep -Ev "$allowed" || true)

if [ -n "$unexpected" ]; then
  echo "the interface gained dependencies it must not have:" >&2
  echo "$unexpected" >&2
  exit 1
fi

echo "the interface depends on serialization and error crates only"
