//! Preparation of portable task-query vocabulary.

#[cfg(test)]
#[doc = "Compatibility tests for the compiled connector vocabulary."]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[path = "tasks_tests.rs"]
mod tests;

#[cfg(test)]
use tinyconnectors_bus::vocabulary::{
    GithubFetchMode, NormalizedTask, TaskContainer, TaskFetchFilter, TaskKind,
};
