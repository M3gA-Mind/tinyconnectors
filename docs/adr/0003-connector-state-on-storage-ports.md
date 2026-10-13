# 0003: Connector state goes through tinystoragedrivers ports

Status: accepted (tinyhumansai/openhuman#7522, #7641)

Scope preferences and the trigger archive no longer write files.

- Preferences: `DocumentPrefsStore` over a `DocumentStore`, collection
  `composio-user-scopes`, document id = toolkit key, body `{ "value": <pref> }`.
- Trigger archive: `TriggerArchive` over a `StreamStore`, one stream per UTC day
  named `triggers/<YYYY-MM-DD>`. The port assigns offsets atomically, so the
  file lock (and `fs4`) is gone.
- The crate depends on `tinystoragedrivers-core` only (no driver, no database
  client).
- The module reaches the ports over the host's storage bus
  (`tinystoragedrivers-busproxy::connect_storage`, as tinychannels does), lazily
  and scoped by the host from the authenticated caller. The process-local
  `register_storage` / `ModuleStorage` registry is removed: a cdylib has its own
  statics, so a host registry never reached it. The configuration's `state_dir`
  and `OpenArchiveRequest.state_dir` are accepted and ignored for wire
  compatibility.
- The host grant is `tinyconnectors::state::STORAGE_NEEDS`: collection
  `composio-user-scopes`, stream prefix `triggers/`. The module never calls
  `ensure_collection`.
- With no storage served, preferences stay in memory (each fall-back is logged)
  and trigger history / archive leases report storage unavailable.

Deferred: importing the legacy `<state_dir>/composio-user-scopes/*.json` and
`<state_dir>/triggers/*.jsonl` files and the host-side grant are in OpenHuman
(#7710, #7636).
