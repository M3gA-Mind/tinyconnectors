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
- A host registers the ports with `register_storage(key, ModuleStorage)`. The
  module configuration's `state_dir` (and `OpenArchiveRequest.state_dir`) is now
  that key; the module never touches the directory. An unregistered key gives
  in-memory preferences and "trigger archive storage unavailable".

Deferred: the cdylib build has its own registry, so a host loading it over
TinyBus needs the storage bus client (tinystoragedrivers-bus) wired in the host
adapter; importing the legacy `<state_dir>/composio-user-scopes/*.json` and
`<state_dir>/triggers/*.jsonl` files is host-side (openhuman#7636).
