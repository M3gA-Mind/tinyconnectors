# Restricted module initialization

## Publication dependency

This implementation is complete and locally verified, but publication is deliberately a draft dependency PR until https://github.com/tinyhumansai/tinybus/pull/49 is merged into canonical main. The child gitlink is pinned exactly to its verified head `eb1c7448b9cdea72dd4e9d4281f8f0739f0308df`; the workspace uses TinyBus 0.1.3. No release or downstream host consumption is authorized or claimed.

When PR49 is upstream available, confirm the pinned commit is in canonical history, rerun dependency admission, and mark this PR ready. Preserve all individual commits.

## Runtime behavior

Contract minor 1.15 adds RuntimeMode, reporting the actual instance mode and contract version. A safe per-instance setup factory seals the first admitted mode. Reinitialization within that mode remains compatible; attempts to change it fail before setup can replace the service or open state.

Classic mode retains its existing route, provider, preferences and archive behavior. The explicit `routed_read_only` mode rejects stored routes and state directories during configuration parsing. Its separate interface has no client slot, preference store or archive. It supports:

- Confidential RoutedRead for the five existing stateless toolkit, connection and trigger reads.
- RuntimeMode and framework introspection.
- Empty ListCapabilities, reflecting unavailable provider/stateful operations.
- ListAgentReadyToolkits from the static compiled catalog.

Every other member is denied before payload decoding. Ordinary delivery of RoutedRead remains refused, even on this separate interface. The immutable per-call route owns its local client for the whole operation and never uses Configure or shared credential/cache markers. Stateful operations and provider policy remain unavailable pending admitted owned state; no supplied model policy or default preferences substitute for that authority.

## Regression evidence

Actual compiled module mode-sealing tests failed with the legacy callback, including after selecting corrected SDK49. Three regressions were then confirmed against ready instances: restricted-to-classic transition, independent classic/restricted modes with compatible same-mode reinitialization, and refusal before state opening. An initial fixture that raced module readiness was corrected before recording the final RED. The per-instance factory seal makes all three GREEN.

Other tests cover readonly directory nonmutation, all denied members, confidentiality-required delivery, truthful capabilities, initialization validation, contract version/serde compatibility and classic behavior. The dynamic verifier loads classic and restricted modes in separate processes, because the cdylib owns a process-global tracing subscriber. A restricted dynamic run with a read-only working directory exited0 and left the directory empty.

## Fresh verification

All commands completed successfully in the standalone owning worktree, without changing OpenHuman's shared dependencies:

- Strict workspace Clippy, all targets/features, warnings denied.
- Full workspace build and test with all features: module 305 unit tests, bus 67, sync 62, six integration tests, four doctests.
- Default workspace: module 302 unit tests, bus 67, sync 62, six integration tests, four doctests.
- Restricted focused suite: seven tests, including three compiled mode regressions.
- Rust 1.88 full workspace/all targets/all features build.
- Rustdoc warnings denied, stub example, dynamic module verifier, no-default check, fmt and diff checks.
- Contract dependency tree contains no transport, runtime or native storage dependency.
- Owning per-file coverage gate: 42 measured source files, minimum 91.3793%; module adapter 95.01%, restricted interface 96.97%.

Logs are under `target/debug-logs/restricted-*`. Final behavior RED is `restricted-mode-all-red-final.log`; corresponding GREEN is `restricted-mode-sealing-green.log`. This tracked summary preserves the result if build output is cleaned.
