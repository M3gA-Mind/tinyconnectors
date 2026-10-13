//! Walks over JSON Schema and JSON values that find where a tool's list lives
//! and which fields it declares.
//!
//! Vendor-neutral: the one piece of provider knowledge a walk needs, the
//! response-envelope keys to skip at the root, is a parameter. These are the
//! same walks the agent harness ships; they are repeated here only because the
//! module must not depend on the harness. They are small and pure.

use serde_json::Value;

