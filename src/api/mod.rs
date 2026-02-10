//! API module
//!
//! HTTP client for communicating with the PC Builder backend API.

mod client;

pub use client::{submit_benchmark, ApiError};
