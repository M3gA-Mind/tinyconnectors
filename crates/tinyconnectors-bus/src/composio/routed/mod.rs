//! Confidential per-call immutable routes for reads that need no stored policy.
//!
//! Hosts construct these requests from admitted context and send them through
//! confidential `TinyBus` calls. No scope preference, archive, execution policy or
//! provider state is accepted by this API. Legacy Configure remains independent.

mod types;
pub use types::{
    ComposioCallRoute, ComposioReadOperation, ComposioRoutedReadReply, ComposioRoutedReadRequest,
};

#[cfg(test)]
#[path = "mod_tests.rs"]
#[allow(clippy::expect_used)]
mod tests;
