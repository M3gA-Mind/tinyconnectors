//! Preparation of portable task-query vocabulary.

use tinyconnectors_bus::vocabulary::{GithubFetchMode, NormalizedTask, TaskContainer, TaskFetchFilter, TaskKind};

/// Default an omitted query cap to 25.
pub(crate) trait TaskLimit { fn effective_max(&self) -> usize; }
impl TaskLimit for TaskFetchFilter { fn effective_max(&self) -> usize { if self.max == 0 { 25 } else { self.max as usize } } }

#[cfg(test)]
#[path = "tasks_tests.rs"]
mod tests;
