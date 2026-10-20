#!/usr/bin/env bash
set -euo pipefail

native_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
super_root="$(cd -- "$native_root/../.." && pwd)"
git_common="$(git -C "$super_root" rev-parse --path-format=absolute --git-common-dir)"
archive="$native_root/target/legacy-v0.21.0-source"

rm -rf -- "$archive"
mkdir -p -- "$archive" "$archive/vendor/tinybus" "$archive/crates/tinyconnectors/examples"
git -C "$native_root" archive 36456935ba86b683e1e56871e975c4c3bd014a2a | tar -x -C "$archive"
git -C "$native_root/vendor/tinybus" archive HEAD | tar -x -C "$archive/vendor/tinybus"
cp -- "$native_root/scripts/fixtures/legacy_storage_writer.rs" \
    "$archive/crates/tinyconnectors/examples/legacy_storage_writer.rs"

cd -- "$archive"
mkdir -p -- target/compiler-scratch
env TMPDIR="$archive/target/compiler-scratch" RUSTC_WRAPPER= CARGO_BUILD_JOBS=1 \
    flock "$git_common/implementing-an-epic/8131-cargo.lock" \
    "$super_root/scripts/ci-cancel-aware.sh" \
    cargo run --locked --no-default-features -p tinyconnectors \
        --example legacy_storage_writer
