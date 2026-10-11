//! Preparation of portable task-query vocabulary.

use tinyconnectors_bus::vocabulary::TaskFetchFilter;

/// Default an omitted query cap to 25.
pub(crate) trait TaskLimit {
    fn effective_max(&self) -> usize;
}
impl TaskLimit for TaskFetchFilter {
    fn effective_max(&self) -> usize {
        if self.max == 0 { 25 } else { self.max as usize }
    }
}

#[cfg(test)]
#[doc = "Compatibility tests for the compiled connector vocabulary."]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[path = "tasks_tests.rs"]
mod tests;

#[cfg(test)]
#[doc = "Compatibility tests for the compiled connector vocabulary."]
use tinyconnectors_bus::vocabulary::{GithubFetchMode, NormalizedTask, TaskContainer, TaskKind};
