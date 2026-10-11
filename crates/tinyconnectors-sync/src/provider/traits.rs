//! The provider trait and its value types.

use super::context::ProviderContext;
use crate::Result;
use crate::scope::CuratedTool;
use async_trait::async_trait;

pub use tinyconnectors_bus::vocabulary::ProviderUserProfile;

/// What a connector knows about one toolkit.
#[async_trait]
pub trait ConnectorProvider: Send + Sync + std::fmt::Debug {
    /// Toolkit slug, e.g. `"gmail"`.
    ///
    /// Must match the slug Composio uses: the registry keys on it, and a
    /// mismatch means the provider is simply never found.
    fn toolkit_slug(&self) -> &'static str;

    /// A one-line description of what connecting this toolkit gets the user.
    fn description(&self) -> &'static str;

    /// The actions worth offering an agent, if this provider curates them.
    ///
    /// `None` means uncurated: every action passes through, and scope gating
    /// falls back to [`crate::classify_unknown`].
    fn curated_tools(&self) -> Option<&'static [CuratedTool]> {
        None
    }

    /// Read the connected account's identity.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Action`] when the underlying action fails.
    async fn fetch_user_profile(&self, context: &ProviderContext) -> Result<ProviderUserProfile>;
}
