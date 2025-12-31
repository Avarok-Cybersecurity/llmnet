//! System metrics collection for worker nodes
//!
//! This module provides functionality to collect system metrics (CPU, memory,
//! disk, GPU) and request statistics for reporting to the control plane.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use chrono::Utc;
use sysinfo::{Disks, System};

use crate::cluster::node::NodeMetrics;

/// Metrics collector for a worker node
///
/// Collects system metrics using the `sysinfo` crate and tracks request
/// statistics via atomic counters for thread-safe updates.
pub struct MetricsCollector {
    system: System,
    disks: Disks,

    // Request tracking (updated by request handlers)
    request_count: AtomicU64,
    active_requests: AtomicU32,
    total_latency_ms: AtomicU64,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            system: System::new_all(),
            disks: Disks::new_with_refreshed_list(),
            request_count: AtomicU64::new(0),
            active_requests: AtomicU32::new(0),
            total_latency_ms: AtomicU64::new(0),
        }
    }

    /// Refresh system metrics and return NodeMetrics
    ///
    /// This should be called periodically (e.g., every 30 seconds) to
    /// collect fresh metrics for the heartbeat.
    pub fn collect(&mut self) -> NodeMetrics {
        // Refresh CPU and memory info
        self.system.refresh_cpu_all();
        self.system.refresh_memory();
        self.disks.refresh(true);

        // CPU usage (global average across all cores)
        let cpu_usage = self.system.global_cpu_usage() as f64;

        // Memory usage
        let total_mem = self.system.total_memory();
        let used_mem = self.system.used_memory();
        let memory_usage = if total_mem > 0 {
            (used_mem as f64 / total_mem as f64) * 100.0
        } else {
            0.0
        };

        // Disk usage (sum of all disks)
        let (total_disk, used_disk) = self
            .disks
            .iter()
            .map(|d| (d.total_space(), d.total_space() - d.available_space()))
            .fold((0u64, 0u64), |(t, u), (dt, du)| (t + dt, u + du));
        let disk_usage = if total_disk > 0 {
            (used_disk as f64 / total_disk as f64) * 100.0
        } else {
            0.0
        };

        // GPU metrics (requires feature flag)
        let (gpu_usage, gpu_memory_usage) = self.collect_gpu_metrics();

        // Request metrics - swap to reset counters
        let req_count = self.request_count.swap(0, Ordering::SeqCst);
        let active = self.active_requests.load(Ordering::SeqCst);
        let total_latency = self.total_latency_ms.swap(0, Ordering::SeqCst);
        let avg_latency = if req_count > 0 {
            total_latency as f64 / req_count as f64
        } else {
            0.0
        };

        NodeMetrics {
            cpu_usage_percent: cpu_usage,
            memory_usage_percent: memory_usage,
            gpu_usage_percent: gpu_usage,
            gpu_memory_usage_percent: gpu_memory_usage,
            disk_usage_percent: disk_usage,
            request_count: req_count,
            avg_latency_ms: avg_latency,
            active_requests: active,
            collected_at: Utc::now(),
        }
    }

    /// Collect GPU metrics (NVIDIA only, requires `gpu` feature)
    #[cfg(feature = "gpu")]
    fn collect_gpu_metrics(&self) -> (Option<f64>, Option<f64>) {
        use nvml_wrapper::Nvml;

        match Nvml::init() {
            Ok(nvml) => {
                // Try to get first GPU
                if let Ok(device) = nvml.device_by_index(0) {
                    let utilization = device.utilization_rates().map(|u| u.gpu as f64).ok();
                    let memory = device
                        .memory_info()
                        .map(|m| (m.used as f64 / m.total as f64) * 100.0)
                        .ok();
                    return (utilization, memory);
                }
                (None, None)
            }
            Err(_) => (None, None),
        }
    }

    /// Collect GPU metrics - stub when GPU feature is disabled
    #[cfg(not(feature = "gpu"))]
    fn collect_gpu_metrics(&self) -> (Option<f64>, Option<f64>) {
        (None, None)
    }

    /// Record the start of a request
    ///
    /// Call this when a request begins processing. Returns a guard that
    /// should be dropped when the request completes.
    pub fn record_request_start(&self) {
        self.active_requests.fetch_add(1, Ordering::SeqCst);
    }

    /// Record the end of a request with its latency
    ///
    /// Call this when a request completes, passing the latency in milliseconds.
    pub fn record_request_end(&self, latency_ms: u64) {
        self.active_requests.fetch_sub(1, Ordering::SeqCst);
        self.request_count.fetch_add(1, Ordering::SeqCst);
        self.total_latency_ms
            .fetch_add(latency_ms, Ordering::SeqCst);
    }

    /// Get the current number of active requests
    pub fn active_requests(&self) -> u32 {
        self.active_requests.load(Ordering::SeqCst)
    }

    /// Get the total request count since last collection
    pub fn request_count(&self) -> u64 {
        self.request_count.load(Ordering::SeqCst)
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared metrics collector for use across async tasks
pub type SharedMetricsCollector = Arc<tokio::sync::RwLock<MetricsCollector>>;

/// Create a new shared metrics collector
pub fn new_shared_collector() -> SharedMetricsCollector {
    Arc::new(tokio::sync::RwLock::new(MetricsCollector::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_collector() {
        let collector = MetricsCollector::new();
        assert_eq!(collector.active_requests(), 0);
        assert_eq!(collector.request_count(), 0);
    }

    #[test]
    fn test_request_tracking() {
        let collector = MetricsCollector::new();

        // Start two requests
        collector.record_request_start();
        collector.record_request_start();
        assert_eq!(collector.active_requests(), 2);

        // Complete one request
        collector.record_request_end(100);
        assert_eq!(collector.active_requests(), 1);
        assert_eq!(collector.request_count(), 1);

        // Complete second request
        collector.record_request_end(200);
        assert_eq!(collector.active_requests(), 0);
        assert_eq!(collector.request_count(), 2);
    }

    #[test]
    fn test_collect_metrics() {
        let mut collector = MetricsCollector::new();

        // Record some requests first
        collector.record_request_start();
        collector.record_request_end(100);
        collector.record_request_start();
        collector.record_request_end(200);

        let metrics = collector.collect();

        // CPU and memory should be non-negative
        assert!(metrics.cpu_usage_percent >= 0.0);
        assert!(metrics.memory_usage_percent >= 0.0);
        assert!(metrics.disk_usage_percent >= 0.0);

        // Request stats should reflect what we recorded
        assert_eq!(metrics.request_count, 2);
        assert_eq!(metrics.avg_latency_ms, 150.0); // (100 + 200) / 2

        // After collection, counters should be reset
        assert_eq!(collector.request_count(), 0);
    }

    #[test]
    fn test_metrics_defaults() {
        let metrics = NodeMetrics::default();
        assert_eq!(metrics.cpu_usage_percent, 0.0);
        assert_eq!(metrics.memory_usage_percent, 0.0);
        assert_eq!(metrics.request_count, 0);
    }
}
