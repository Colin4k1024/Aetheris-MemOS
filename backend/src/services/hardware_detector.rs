//! Hardware capability detection for heterogeneous compute environments.
//!
//! Detects CUDA GPUs, Apple Metal / Apple Silicon (with Neural Engine),
//! and CPU resources. Results are cached in a `OnceLock` and used by
//! `model_router` to select the best embedding / LLM configuration.

use std::sync::OnceLock;

use sysinfo::{CpuExt, System, SystemExt};
use tracing::info;

/// Detected hardware capabilities of the host machine.
#[derive(Debug, Clone)]
pub struct HardwareCapabilities {
    /// True if at least one NVIDIA GPU was found (via `nvidia-smi` or `/dev/nvidia0`).
    pub has_cuda: bool,
    /// Number of CUDA devices detected (`0` when `has_cuda` is false).
    pub cuda_device_count: u32,
    /// Total VRAM across all CUDA devices in MB. May be `0` when the count is
    /// unknown (filesystem probe fallback).
    pub cuda_total_vram_mb: u64,
    /// True on macOS — Metal GPU is always available there.
    pub has_metal: bool,
    /// True when the binary is compiled for `aarch64-apple-darwin` (Apple Silicon).
    pub is_apple_silicon: bool,
    /// True when an Apple Neural Engine is present (= `is_apple_silicon`).
    pub has_npu: bool,
    /// Number of physical CPU cores.
    pub cpu_physical_cores: u32,
    /// Total system RAM in MB.
    pub total_ram_mb: u64,
}

impl HardwareCapabilities {
    /// Short tag describing the best available compute backend.
    pub fn best_backend_tag(&self) -> &'static str {
        if self.has_cuda && self.cuda_total_vram_mb >= 8192 {
            "cuda-high-vram"
        } else if self.has_cuda && self.cuda_total_vram_mb > 0 {
            "cuda-low-vram"
        } else if self.has_cuda {
            "cuda"
        } else if self.is_apple_silicon {
            "apple-silicon"
        } else if self.has_metal {
            "metal"
        } else {
            "cpu"
        }
    }
}

static HARDWARE: OnceLock<HardwareCapabilities> = OnceLock::new();

/// Detect hardware capabilities at startup and cache the result.
///
/// Subsequent calls are no-ops; use [`get`] to retrieve the cached value.
pub fn init() {
    if HARDWARE.get().is_some() {
        return;
    }
    let caps = detect();
    info!(
        backend = caps.best_backend_tag(),
        has_cuda = caps.has_cuda,
        cuda_devices = caps.cuda_device_count,
        cuda_vram_mb = caps.cuda_total_vram_mb,
        apple_silicon = caps.is_apple_silicon,
        has_npu = caps.has_npu,
        cpu_cores = caps.cpu_physical_cores,
        ram_mb = caps.total_ram_mb,
        "Hardware capabilities detected"
    );
    let _ = HARDWARE.set(caps);
}

/// Return a reference to the cached [`HardwareCapabilities`].
///
/// Returns `None` if [`init`] has not been called yet.
pub fn get() -> Option<&'static HardwareCapabilities> {
    HARDWARE.get()
}

/// Perform hardware detection without caching. Prefer [`init`] + [`get`] for
/// production use since this runs a subprocess (`nvidia-smi`) on each call.
pub fn detect() -> HardwareCapabilities {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_physical_cores = sys.physical_core_count().unwrap_or(1) as u32;
    let total_ram_mb = sys.total_memory() / 1024 / 1024;

    // CUDA detection via nvidia-smi or Linux filesystem probe
    let (has_cuda, cuda_device_count, cuda_total_vram_mb) = detect_cuda();

    // Metal / Apple Silicon by compile-time target
    let has_metal = cfg!(target_os = "macos");
    let is_apple_silicon = cfg!(all(target_os = "macos", target_arch = "aarch64"));
    // The Apple Neural Engine is present on every Apple Silicon chip.
    let has_npu = is_apple_silicon;

    HardwareCapabilities {
        has_cuda,
        cuda_device_count,
        cuda_total_vram_mb,
        has_metal,
        is_apple_silicon,
        has_npu,
        cpu_physical_cores,
        total_ram_mb,
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Attempt to detect NVIDIA CUDA GPUs without linking against CUDA drivers.
///
/// Strategy:
/// 1. Run `nvidia-smi --query-gpu=memory.total --format=csv,noheader,nounits`
///    and parse per-GPU VRAM values (MB).
/// 2. On Linux, fall back to checking for `/dev/nvidia0`.
fn detect_cuda() -> (bool, u32, u64) {
    // Primary: nvidia-smi query
    match std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let vram_values: Vec<u64> = text
                .lines()
                .filter_map(|l| l.trim().parse::<u64>().ok())
                .collect();
            if !vram_values.is_empty() {
                let count = vram_values.len() as u32;
                let total: u64 = vram_values.iter().sum();
                return (true, count, total);
            }
        }
        _ => {}
    }

    // Fallback: filesystem probe (Linux, no nvidia-smi in PATH)
    #[cfg(target_os = "linux")]
    if std::path::Path::new("/dev/nvidia0").exists() {
        return (true, 1, 0);
    }

    (false, 0, 0)
}
