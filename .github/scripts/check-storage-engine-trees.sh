#!/usr/bin/env bash
set -euo pipefail

blocked='rusqlite|libsqlite3-sys|mongodb|bson|aws-sdk-s3|lapin|amq-protocol|sqlx|redb|sled|rocksdb|^tinystoragedrivers$|tinystoragedrivers-(core|sqlite|mongodb|document|kv|blob|queue|secrets|connect|testkit)'

has_blocked_dependency() {
  awk '{print $1}' | grep -xE "$blocked"
}

normalize_text_output() {
  tr -d '\r'
}

self_test() {
  local benign engine fixture normalized selected
  normalized=$(printf 'native-lib\r\n' | normalize_text_output)
  [[ "$normalized" == 'native-lib' ]] || {
    printf 'storage-engine dependency check failed CRLF normalization self-test: %q\n' "$normalized" >&2
    return 1
  }
  benign=$(printf '%s\n' \
    'tinyconnectors v0.21.0' \
    'tinystoragedrivers-api v2.0.1' \
    'tinystoragedrivers-busproxy v2.0.1' \
    'tinystoragedrivers-types v2.0.1')
  if has_blocked_dependency <<<"$benign" >/dev/null; then
    echo 'storage-engine dependency check rejected a port or testkit crate' >&2
    return 1
  fi
  for engine in \
    'rusqlite v0.31.0' \
    'tinystoragedrivers v2.0.1' \
    'tinystoragedrivers-mongodb v1.0.0' \
    'tinystoragedrivers-document v2.0.1' \
    'tinystoragedrivers-core v1.0.0' \
    'tinystoragedrivers-connect v2.0.1' \
    'tinystoragedrivers-kv v2.0.1' \
    'tinystoragedrivers-blob v2.0.1' \
    'tinystoragedrivers-queue v2.0.1' \
    'tinystoragedrivers-secrets v2.0.1' \
    'tinystoragedrivers-testkit v2.0.1'; do
    if ! has_blocked_dependency <<<"$engine" >/dev/null; then
      printf 'storage-engine dependency check did not reject %s\n' "$engine" >&2
      return 1
    fi
  done

  fixture='{"workspace_members":["id-lib","id-rlib","id-dylib","id-cdylib","id-staticlib","id-proc"],"packages":[{"id":"id-lib","name":"native-lib","targets":[{"kind":["lib"]}]},{"id":"id-rlib","name":"native-rlib","targets":[{"kind":["rlib"]}]},{"id":"id-dylib","name":"native-dylib","targets":[{"kind":["dylib"]}]},{"id":"id-cdylib","name":"native-cdylib","targets":[{"kind":["cdylib"]}]},{"id":"id-staticlib","name":"native-staticlib","targets":[{"kind":["staticlib"]}]},{"id":"id-proc","name":"native-proc-macro","targets":[{"kind":["proc-macro"]}]},{"id":"id-external","name":"external-workspace-package","targets":[{"kind":["cdylib"]}]}]}'
  selected=$(jq -r --argjson members '["id-lib","id-rlib","id-dylib","id-cdylib","id-staticlib","id-proc"]' '
    .packages[]
    | select(.id as $id | $members | index($id))
    | select(any(.targets[]; any(.kind[]; IN("lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro"))))
    | .name
  ' <<<"$fixture" | sed 's/$/\r/' | normalize_text_output | sort)
  [[ "$selected" == $'native-cdylib\nnative-dylib\nnative-lib\nnative-proc-macro\nnative-rlib\nnative-staticlib' ]] || {
    printf 'workspace library target selection failed self-test: %s\n' "$selected" >&2
    return 1
  }
}

self_test

metadata=$(cargo metadata --no-deps --format-version 1)
packages=$(jq -r '
  . as $metadata
  | .packages[]
  | select(.id as $id | $metadata.workspace_members | index($id))
  | select(any(.targets[]; any(.kind[]; IN("lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro"))))
  | .name
' <<<"$metadata" | normalize_text_output | sort -u)

if [[ -z "$packages" ]]; then
  echo 'no TinyConnectors library targets found in workspace metadata' >&2
  exit 1
fi

while IFS= read -r package; do
  [[ -n "$package" ]] || continue
  tree=$(cargo tree -p "$package" --all-features -e normal,build --prefix none)
  tree_root=$(head -n 1 <<<"$tree" | awk '{print $1}')
  if [[ "$tree_root" != "$package" ]]; then
    printf 'normal/build dependency tree for %s was empty or rooted at %s\n' \
      "$package" "${tree_root:-<empty>}" >&2
    exit 1
  fi
  if matches=$(has_blocked_dependency <<<"$tree"); then
    printf 'normal/build dependency tree for %s contains a storage engine:\n%s\n' "$package" "$matches" >&2
    exit 1
  fi
  printf 'checked %s\n' "$package"
done <<<"$packages"
