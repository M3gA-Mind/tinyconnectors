# Legacy storage fixtures

`v0.21.0-gmail-pref.json` is output from the public `DocumentPrefsStore` and
`PrefsStore::set` APIs in TinyConnectors tag `v0.21.0` at
`36456935ba86b683e1e56871e975c4c3bd014a2a`. Its TinyBus gitlink is
`1f33d1be1901a2d44bfe674cd7cf294cf3848821`, and its lockfile pins TSD
`v1.0.0` to `141fd9ed0ad5d14a8fb5a1b3e837a719e81ac08d`. The artifact SHA-256
is `11c9c8c63cb1703cb5daea1819c52f1a34d9a35ac31e4556117665e8f4110d25`. It
contains the prior release's current preference record; separate migration
tests cover older legacy-schema transformations.

The module-open compatibility test loads this record through the actual module
bus path and checks that its document body, schema, and existing host-assigned
creation timestamp remain unchanged. Cross-collection legacy-schema migration
preserves the preference payload, schema, and absolute expiry. The host assigns
`created_ms` for that new destination record; TSD intentionally forbids modules
from overriding that host-owned field.

To regenerate it, run `scripts/fixtures/generate-legacy-v0.21.0.sh` from the
TinyConnectors checkout. The script archives the exact tag into the ignored
`target/legacy-v0.21.0-source` directory, adds the checked-in public-writer
example to `crates/tinyconnectors/examples/`, copies the TinyBus source pinned
by the old workspace, and runs:

```sh
cargo run --locked --no-default-features -p tinyconnectors \
  --example legacy_storage_writer
```

The example is compiled with the archived workspace lockfile and the archived
TSD `v1.0.0` dependency. Its JSON output is the fixture bytes. No current
TinyConnectors or TSD implementation writes the artifact. `created_ms` and
`updated_ms` come from the old writer's host clock, so regeneration at another
time changes those values and the fixture hash.
