//! Immutable module initialization mode admission.
mod types;
pub use types::{ComposioRuntimeMode, ComposioRuntimeModeResponse};

#[cfg(test)]
#[path = "mod_tests.rs"]
#[allow(clippy::expect_used)]
mod tests;
