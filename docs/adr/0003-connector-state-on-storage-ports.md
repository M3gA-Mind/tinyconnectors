# 0003: Connector state goes through tinystoragedrivers ports

Status: accepted (tinyhumansai/openhuman#7522, #7641)

Scope preferences and the trigger archive no longer write files.

- Preferences: `DocumentPrefsStore` over a `DocumentStore`, collection
  `connectors.user_scopes`, document id = legacy composite key, body the preference.
- Trigger archive: `TriggerArchive` over a `StreamStore`, one stream per UTC day
  named `connectors.triggers/<YYYY-MM-DD>` (`triggers/<YYYY-MM-DD>` before schema v2). The port assigns offsets atomically, so the
  file lock (and `fs4`) is gone.
- The crate's normal storage dependencies are `tinystoragedrivers-types` and
  `tinystoragedrivers-api` (plus `tinystoragedrivers-busproxy` for its module).
- The module reaches the ports over the host's storage bus
  (`tinystoragedrivers-busproxy::connect_storage`, as tinychannels does), lazily
  and scoped by the host from the authenticated caller. The process-local
  `register_storage` / `ModuleStorage` registry is removed: a cdylib has its own
  statics, so a host registry never reached it. The configuration's `state_dir`
  and `OpenArchiveRequest.state_dir` are accepted and ignored for wire
  compatibility.
- The host provisions the pure `collections()` declarations and grants the
  names returned by `tinyconnectors_bus::storage::storage_needs()`, including
  the legacy source names needed for migration. On its first bound storage
  open, the module runs its own idempotent migration of v1 preferences and
  trigger streams. Preferences live in `connectors.user_scopes` (id uses the
  legacy composite-key encoding; non-object values sit under
  `non_object_value`), and trigger streams use `connectors.triggers/`.
- With no storage served, preference operations fail with an unavailable-store
  error and trigger history reports storage unavailable; no in-memory fallback
  can make a choice appear saved.

Deferred: importing the legacy `<state_dir>/composio-user-scopes/*.json` and
`<state_dir>/triggers/*.jsonl` files and the host-side grant are in OpenHuman
(#7710, #7636).
