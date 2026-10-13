//! Wire tests for the portable task-source configuration.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use super::*;
use serde_json::json;

#[test]
fn provider_slug_roundtrips_and_parses_case_insensitively() {
    for provider in [
        ProviderSlug::Github,
        ProviderSlug::Notion,
        ProviderSlug::Linear,
        ProviderSlug::Clickup,
    ] {
        assert_eq!(ProviderSlug::parse(provider.as_str()).unwrap(), provider);
    }
    assert_eq!(ProviderSlug::parse("GitHub").unwrap(), ProviderSlug::Github);
    assert!(ProviderSlug::parse("jira").is_err());
    assert_eq!(
        serde_json::to_string(&ProviderSlug::Clickup).unwrap(),
        "\"clickup\""
    );
}

#[test]
fn filter_spec_is_tagged_by_provider_and_roundtrips() {
    let spec = FilterSpec::Github {
        repo: Some("owner/name".into()),
        labels: vec!["bug".into()],
        assignee_is_me: true,
        state: Some("open".into()),
        fetch_mode: GithubFetchMode::default(),
        extra: json!({}),
    };
    let wire = serde_json::to_value(&spec).unwrap();
    assert_eq!(wire["provider"], "github");
    assert_eq!(wire["repo"], "owner/name");
    assert_eq!(spec.provider(), ProviderSlug::Github);
    assert_eq!(serde_json::from_value::<FilterSpec>(wire).unwrap(), spec);

    let notion = FilterSpec::Notion {
        database_id: Some("db-1".into()),
        assigned_to_me: true,
        status: Some("In Progress".into()),
        extra: json!({"page_size": 10}),
    };
    let back: FilterSpec = serde_json::from_value(serde_json::to_value(&notion).unwrap()).unwrap();
    assert_eq!(back, notion);
    assert_eq!(back.provider(), ProviderSlug::Notion);
}

#[test]
fn source_target_and_fetch_reason_keep_their_wire_tags() {
    assert_eq!(SourceTarget::default(), SourceTarget::AgentTodoProactive);
    assert_eq!(
        serde_json::to_value(SourceTarget::TodoOnly).unwrap(),
        "todo_only"
    );
    assert_eq!(FetchReason::Periodic.as_str(), "periodic");
    assert_eq!(
        FetchReason::ConnectionCreated.as_str(),
        "connection_created"
    );
    assert_eq!(FetchReason::Manual.as_str(), "manual");
    assert_eq!(
        serde_json::to_value(FetchReason::ConnectionCreated).unwrap(),
        "connection_created"
    );
}

#[test]
fn github_filter_maps_repo_labels_and_assignee() {
    let spec = FilterSpec::Github {
        repo: Some("tinyhumansai/openhuman".into()),
        labels: vec!["bug".into(), "p1".into()],
        assignee_is_me: true,
        state: Some("open".into()),
        fetch_mode: GithubFetchMode::default(),
        extra: json!({"per_page": 10}),
    };
    let filter = spec.to_fetch_filter(25);
    assert_eq!(filter.repo.as_deref(), Some("tinyhumansai/openhuman"));
    assert_eq!(filter.labels, vec!["bug".to_owned(), "p1".to_owned()]);
    assert!(filter.assignee_is_me);
    assert_eq!(filter.state.as_deref(), Some("open"));
    assert_eq!(filter.extra["per_page"], 10);
    assert_eq!(filter.max, 25);
    assert!(filter.database_id.is_none());
    assert!(filter.team_id.is_none());
}

#[test]
fn notion_filter_maps_board_and_status() {
    let spec = FilterSpec::Notion {
        database_id: Some("board-xyz".into()),
        assigned_to_me: true,
        status: Some("Todo".into()),
        extra: json!({}),
    };
    let filter = spec.to_fetch_filter(5);
    assert_eq!(filter.database_id.as_deref(), Some("board-xyz"));
    assert!(filter.assignee_is_me, "assigned_to_me maps to assignee_is_me");
    assert_eq!(filter.status.as_deref(), Some("Todo"));
    assert_eq!(filter.max, 5);
}

#[test]
fn linear_and_clickup_filters_map_their_scopes() {
    let linear = FilterSpec::Linear {
        team_id: Some("team-1".into()),
        assignee_is_me: true,
        state: Some("started".into()),
        extra: json!({}),
    }
    .to_fetch_filter(50);
    assert_eq!(linear.team_id.as_deref(), Some("team-1"));
    assert_eq!(linear.state.as_deref(), Some("started"));
    assert!(linear.assignee_is_me);

    let clickup = FilterSpec::Clickup {
        team_id: Some("ws-1".into()),
        list_id: Some("list-9".into()),
        assignee_is_me: true,
        extra: json!({}),
    }
    .to_fetch_filter(25);
    assert_eq!(clickup.team_id.as_deref(), Some("ws-1"));
    assert_eq!(clickup.list_id.as_deref(), Some("list-9"));
    assert!(clickup.assignee_is_me);
}
