use super::*;

#[test]
fn task_preparation_preserves_provider_extras() {
    let filter = TaskFetchFilter { extra: serde_json::json!({"future":"kept"}), ..Default::default() };
    let prepared = prepare_task_filter(filter.clone());
    assert_eq!(prepared.filter, filter);
    assert_eq!(prepared.effective_max, 25);
}
