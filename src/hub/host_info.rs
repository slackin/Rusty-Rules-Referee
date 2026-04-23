//! Host system facts and metrics gathered via `sysinfo`.

use sysinfo::{Disks, System};

use crate::sync::protocol::{HostInfoPayload, HostMetricSample};

/// Collect static + slow-changing host facts.
pub fn collect_host_info() -> HostInfoPayload {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();

    let cpu_model = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let cpu_cores = sys.cpus().len() as u32;

    let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
    let os = format!(
        "{} {}",
        System::name().unwrap_or_else(|| "unknown".to_string()),
        System::os_version().unwrap_or_default()
    );
    let kernel = System::kernel_version().unwrap_or_else(|| "unknown".to_string());

    let total_ram_bytes = sys.total_memory();

    let disks = Disks::new_with_refreshed_list();
    // Sum total space across all disks (cheap heuristic for "host total disk").
    let disk_total_bytes: u64 = disks.iter().map(|d| d.total_space()).sum();

    HostInfoPayload {
        hostname,
        os,
        kernel,
        cpu_model,
        cpu_cores,
        total_ram_bytes,
        disk_total_bytes,
        public_ip: None,
        external_ip: None,
        urt_installs_json: None,
    }
}

/// Sample current CPU/MEM/DISK/load metrics.
pub fn sample_metrics() -> HostMetricSample {
    let mut sys = System::new();
    sys.refresh_cpu_all();
    // sysinfo recommends refreshing CPU twice with a small delay for accurate %.
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu_all();
    sys.refresh_memory();

    let cpu_pct = sys.global_cpu_usage();

    let mem_pct = if sys.total_memory() > 0 {
        (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0
    } else {
        0.0
    };

    let disks = Disks::new_with_refreshed_list();
    let (total, available): (u64, u64) = disks
        .iter()
        .fold((0, 0), |(t, a), d| (t + d.total_space(), a + d.available_space()));
    let disk_pct = if total > 0 {
        ((total - available) as f32 / total as f32) * 100.0
    } else {
        0.0
    };

    let load = System::load_average();
    let uptime_s = System::uptime();

    HostMetricSample {
        cpu_pct,
        mem_pct,
        disk_pct,
        load1: load.one as f32,
        load5: load.five as f32,
        load15: load.fifteen as f32,
        uptime_s,
    }
}
