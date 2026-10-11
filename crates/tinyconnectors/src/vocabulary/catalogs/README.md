# Compiled Composio vocabulary catalogs

Curated action names, scope categories and capability descriptions extracted
from OpenHuman for issue #7413. The module serves owned copies through
GetVocabulary and computes alias/classification metadata through EvaluateActions.
Hosts link the bus contract alone.

Catalog order, action spelling and descriptions are compatibility surfaces.
The catalog-ready list describes this vocabulary rather than the module's
provider registry. Existing description text is preserved, including historical
references to memory syncing; it does not advertise a new sync operation.

Unknown-scope classification has explicit substring compatibility and leading
verb profiles. Display preferences supplied by a caller never change the
persisted preference enforced by Execute.

See the [extraction specification](../../../../../docs/specs/2026-08-30-connector-extraction.md)
and [sync retirement decision](../../../../../docs/adr/0002-remove-the-sync-engine.md).
