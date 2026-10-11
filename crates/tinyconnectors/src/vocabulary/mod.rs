//! Compiled connector catalogs and portable identity/task normalization.
//!
//! Extracted from `OpenHuman`'s contract vocabulary without changing its stored
//! tags, action names, catalog order, or prompt output. Hosts receive metadata
//! over `TinyBus` and apply their own policy; no operation here executes a tool.

pub(crate) mod catalogs;
pub(crate) mod profile;
pub(crate) mod scopes;
mod tasks;

use profile::IdentityMetadata;
use scopes::ScopePreference;
use tasks::TaskLimit;
use tinyconnectors_bus::vocabulary::{
    ActionVocabularyRequest, ActionVocabularyResponse, IdentityValueResponse,
    NormalizeIdentitiesRequest, NormalizeIdentitiesResponse, PreparedTaskFilter, TaskFetchFilter,
    ToolkitVocabulary, UnknownScopeProfile, VocabularyAction, VocabularyRequest,
    VocabularyResponse,
};

pub(crate) fn get_vocabulary(request: VocabularyRequest) -> VocabularyResponse {
    VocabularyResponse {
        toolkits: request
            .toolkits
            .into_iter()
            .map(|toolkit| ToolkitVocabulary {
                description: catalogs::toolkit_description(&toolkit).into(),
                native_provider: catalogs::has_native_provider(&toolkit),
                actions: catalogs::catalog_for_toolkit(&toolkit).map(|catalog| {
                    catalog
                        .iter()
                        .map(|action| VocabularyAction {
                            slug: action.slug.into(),
                            scope: action.scope,
                        })
                        .collect()
                }),
                toolkit,
            })
            .collect(),
        agent_ready_toolkits: scopes::agent_ready_toolkits()
            .into_iter()
            .map(str::to_owned)
            .collect(),
    }
}

pub(crate) fn evaluate_actions(
    requests: Vec<ActionVocabularyRequest>,
) -> Vec<ActionVocabularyResponse> {
    requests
        .into_iter()
        .map(|request| {
            let toolkit = scopes::toolkit_from_slug(&request.slug);
            let curated_scope = catalogs::curated_scope_for(&request.slug);
            let scope = curated_scope.unwrap_or_else(|| match request.unknown_scope_profile {
                UnknownScopeProfile::SubstringCompatibility => {
                    scopes::classify_unknown(&request.slug)
                }
                UnknownScopeProfile::LeadingVerb => {
                    match tinyconnectors_sync::classify_unknown(&request.slug) {
                        tinyconnectors_sync::ToolScope::Read => {
                            tinyconnectors_bus::vocabulary::ToolScope::Read
                        }
                        tinyconnectors_sync::ToolScope::Write => {
                            tinyconnectors_bus::vocabulary::ToolScope::Write
                        }
                        tinyconnectors_sync::ToolScope::Admin => {
                            tinyconnectors_bus::vocabulary::ToolScope::Admin
                        }
                    }
                }
            });
            let visible = if request.unknown_scope_profile
                == UnknownScopeProfile::SubstringCompatibility
            {
                catalogs::is_action_visible_with_pref(&request.slug, &request.preference)
            } else {
                toolkit.as_deref().is_none_or(|toolkit| {
                    (catalogs::catalog_for_toolkit(toolkit).is_none() || curated_scope.is_some())
                        && request.preference.allows(scope)
                })
            };
            ActionVocabularyResponse {
                toolkit,
                canonical_slug: scopes::canonical_action_slug(&request.slug),
                curated_scope,
                scope,
                visible,
                matches: request
                    .compare_with
                    .as_deref()
                    .map(|other| scopes::action_slug_matches(&request.slug, other)),
            }
        })
        .collect()
}

pub(crate) fn normalize_identities(
    request: NormalizeIdentitiesRequest,
) -> NormalizeIdentitiesResponse {
    NormalizeIdentitiesResponse {
        values: request
            .values
            .into_iter()
            .map(|value| IdentityValueResponse {
                value: profile::canonicalize(value.kind, &value.value),
                confidence: value.kind.confidence(),
                matchable: value.kind.is_matchable(),
            })
            .collect(),
        connection_identifiers: request
            .connection_identifiers
            .iter()
            .map(|value| profile::normalize_connection_identifier(value))
            .collect(),
        prompt_section: profile::render_connected_identities_section(&request.identities),
    }
}

pub(crate) fn prepare_task_filter(filter: TaskFetchFilter) -> PreparedTaskFilter {
    PreparedTaskFilter {
        effective_max: filter.effective_max(),
        filter,
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
