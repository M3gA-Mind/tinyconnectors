# Portable connector vocabulary

This module owns the curated Composio tables and normalization extracted from
OpenHuman for [issue #7413](https://github.com/tinyhumansai/openhuman/issues/7413).
Its implementation stays private. Hosts receive owned data over the Composio
TinyBus interface and link only tinyconnectors-bus.

GetVocabulary returns requested descriptions and action catalogs without
credentials. EvaluateActions returns aliases and display metadata in batches.
The caller selects substring compatibility or leading verb classification.
Catalog classifications take precedence. Scope preferences and provider profiles
use the same contract types as the provider registry. Display metadata cannot change the
persisted preference independently enforced by Execute.

NormalizeIdentities applies durable identifier normalization, confidence and
matching metadata, and the existing prompt renderer. PrepareTaskFilter preserves
the query envelope and supplies the default item cap; it never fetches tasks.

The existing provider registry keeps its own capability and execution behavior.
Catalog-ready vocabulary covers more toolkits than the registered providers and
must not be mistaken for an identity-fetch capability. Task-source fetching was
removed, as described in [ADR 0002](../../../../docs/adr/0002-remove-the-sync-engine.md).

Fixtures in catalogs/fixtures were captured from the original host tables before
migration. They pin every action alias, order, scope and capability description.
The native verify_module example checks the catalog fixture and calls all four
operations through the loaded artifact. Host retirement waits for a published
module release, then needs its own schema and recorded-session acceptance tests.
