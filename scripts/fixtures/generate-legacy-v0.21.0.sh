#!/usr/bin/env bash
set -euo pipefail

native_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
archive="$native_root/target/legacy-v0.21.0-source"
source_sha=36456935ba86b683e1e56871e975c4c3bd014a2a
tinybus_sha="$(git -C "$native_root" ls-tree "$source_sha" vendor/tinybus | awk '{print $3}')"

rm -rf -- "$archive"
mkdir -p -- "$archive" "$archive/vendor/tinybus" "$archive/crates/tinyconnectors/examples"
git -C "$native_root" archive "$source_sha" | tar -x -C "$archive"
git -C "$native_root/vendor/tinybus" archive "$tinybus_sha" | tar -x -C "$archive/vendor/tinybus"
cp -- "$native_root/scripts/fixtures/legacy_storage_writer.rs" \
    "$archive/crates/tinyconnectors/examples/legacy_storage_writer.rs"

cd -- "$archive"
mkdir -p -- target/compiler-scratch
env TMPDIR="$archive/target/compiler-scratch" \
    RUSTC_WRAPPER= CARGO_BUILD_JOBS=1 \
    cargo run --locked --no-default-features -p tinyconnectors \
        --example legacy_storage_writer
