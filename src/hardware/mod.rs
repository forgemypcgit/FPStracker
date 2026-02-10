//! Hardware detection module
//!
//! Detects GPU, CPU, RAM, and other system information using sysinfo
//! and platform-specific APIs (NVML for NVIDIA, sysfs for AMD).

pub mod cpu;
pub mod gpu;
pub mod ram;
mod system;

pub use system::SystemInfo;
