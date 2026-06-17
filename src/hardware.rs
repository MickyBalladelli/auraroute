use std::env;

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::sync::{Mutex, OnceLock};
#[cfg(target_os = "linux")]
use std::time::Instant;

#[cfg(target_os = "linux")]
static CPU_CACHE: OnceLock<Mutex<CpuReading>> = OnceLock::new();

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy)]
struct CpuReading {
    timestamp: Instant,
    total: u64,
    idle: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct SystemResources {
    pub free_vram_mb: u64,
    pub cpu_usage_pct: f32,
}

pub fn get_current_resources() -> SystemResources {
    SystemResources {
        free_vram_mb: configured_free_vram_mb().unwrap_or(8_000),
        cpu_usage_pct: current_cpu_usage_pct().unwrap_or(0.0),
    }
}

pub fn has_local_resource_pressure(complexity_score: u8, resources: &SystemResources) -> bool {
    complexity_score >= 4
        || ((2..=3).contains(&complexity_score) && resources.free_vram_mb < 3_000)
}

fn configured_free_vram_mb() -> Option<u64> {
    env::var("AURAROUTE_FREE_VRAM_MB")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
}

#[cfg(target_os = "linux")]
fn read_cpu_fields() -> Option<(u64, u64)> {
    let stat = fs::read_to_string("/proc/stat").ok()?;
    let first_line = stat.lines().next()?;
    let mut fields = first_line.split_whitespace();

    if fields.next()? != "cpu" {
        return None;
    }

    let values: Vec<u64> = fields.filter_map(|f| f.parse::<u64>().ok()).collect();

    if values.len() < 4 {
        return None;
    }

    let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
    let total: u64 = values.iter().sum();

    Some((total, idle))
}

#[cfg(target_os = "linux")]
fn current_cpu_usage_pct() -> Option<f32> {
    let cache = CPU_CACHE.get_or_init(|| {
        Mutex::new(CpuReading {
            timestamp: Instant::now(),
            total: 0,
            idle: 0,
        })
    });

    let (total, idle) = read_cpu_fields()?;

    let mut cached = cache.lock().ok()?;
    let now = Instant::now();
    let interval = now.duration_since(cached.timestamp).as_secs_f32();

    // On first call, store the baseline and return 0
    if cached.total == 0 {
        cached.timestamp = now;
        cached.total = total;
        cached.idle = idle;
        return Some(0.0);
    }

    // Require at least 1 second between samples for meaningful delta
    if interval < 1.0 {
        return None;
    }

    let delta_total = total.saturating_sub(cached.total);
    let delta_idle = idle.saturating_sub(cached.idle);

    cached.timestamp = now;
    cached.total = total;
    cached.idle = idle;

    if delta_total == 0 {
        return None;
    }

    let busy = delta_total.saturating_sub(delta_idle);
    Some((busy as f32 / delta_total as f32) * 100.0)
}

#[cfg(not(target_os = "linux"))]
fn current_cpu_usage_pct() -> Option<f32> {
    env::var("AURAROUTE_CPU_USAGE_PCT")
        .ok()
        .and_then(|value| value.trim().parse::<f32>().ok())
}