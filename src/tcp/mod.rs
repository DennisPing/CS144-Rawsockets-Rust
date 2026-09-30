//! TCP layer: segment parsing, building, and options handling.
//!
//! Two-layer design:
//! - `TcpSegment` - Owned data model with builder methods
//! - `TcpView` - Zero-copy view for parsing
//! - `TcpOptions` / `TcpOptionsView` - Same pattern for options

pub mod error;
pub mod flags;
pub mod options;
pub mod segment;
pub mod view;

// Re-exports
pub use error::{BuildError, WireError};
pub use flags::TcpFlags;
pub use options::TcpOptions;
pub use segment::TcpSegment;
pub use view::TcpView;
