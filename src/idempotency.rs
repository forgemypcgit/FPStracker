//! Idempotency key helpers.
//!
//! Goal: make retries safe (no accidental duplicate submissions) while keeping key
//! generation consistent across CLI, local API routes, and offline storage.

use uuid::Uuid;

const SUBMIT_PREFIX: &str = "fps-tracker-submit-";
const FEEDBACK_PREFIX: &str = "fps-tracker-feedback-";
const LEGACY_PENDING_PREFIX: &str = "fps-tracker-pending-";
const LEGACY_PENDING_FEEDBACK_PREFIX: &str = "fps-tracker-pending-feedback-";

pub fn new_submit_key() -> String {
    format!("{SUBMIT_PREFIX}{}", Uuid::new_v4().simple())
}

pub fn new_feedback_key() -> String {
    format!("{FEEDBACK_PREFIX}{}", Uuid::new_v4().simple())
}

/// Backward-compatible idempotency key for legacy pending records that didn't
/// persist the original submit key.
pub fn legacy_pending_key(pending_id: &str) -> String {
    format!("{LEGACY_PENDING_PREFIX}{pending_id}")
}

/// Backward-compatible idempotency key for legacy pending feedback records.
pub fn legacy_pending_feedback_key(pending_id: &str) -> String {
    format!("{LEGACY_PENDING_FEEDBACK_PREFIX}{pending_id}")
}
