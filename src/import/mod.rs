//! Import module
//!
//! Parses benchmark data from external FPS overlay tools:
//! - CapFrameX (Windows) - CSV format
//! - MangoHud (Linux) - CSV format
//! - FrameView (Windows) - CSV format (similar to CapFrameX)

pub mod capframex;
mod common;
pub mod mangohud;

pub use capframex::parse_capframex_csv;
pub(crate) use common::FrameData;
pub use mangohud::parse_mangohud_log;
