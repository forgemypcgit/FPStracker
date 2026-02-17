//! API module
//!
//! HTTP client for communicating with the backend API.

mod client;

pub use client::should_queue_offline_feedback;
pub use client::submit_benchmark_with_idempotency_key;
pub use client::submit_feedback_with_idempotency_key;
pub use client::{should_queue_offline, ApiError};
