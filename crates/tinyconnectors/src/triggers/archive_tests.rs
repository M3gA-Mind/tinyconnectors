use super::RETENTION;

#[test]
fn only_streams_idle_beyond_the_policy_window_are_sealed() {
    assert!(RETENTION.should_seal(10_000, Some(10_000 - 172_800_000)));
    assert!(!RETENTION.should_seal(10_000, Some(10_000 - 172_799_999)));
    assert!(!RETENTION.should_seal(10_000, None));
}
