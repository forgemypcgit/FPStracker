//! Benchmark module
//!
//! Handles benchmark data structures, session tracking, and submission logic.

pub mod focus;
pub mod live;
mod session;
pub mod submit;

pub use submit::{BenchmarkSubmission, SubmissionResponse};
