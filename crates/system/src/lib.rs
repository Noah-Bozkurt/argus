use anyhow::Result;
use chrono::Utc;
use protocol::{ServiceState, SystemSnapshot};
use std::process::Command;
use sysinfo::{Disks, System};
use uuid::Uuid;

pub fn collect_snapshot(server_id: Uuid, agent_version: String) -> SystemSnapshot {
    let mut system = System::new_all();
    system.refresh_all();

    let cpu_percent = system.global_cpu_usage();
    let ram_percent = if system.total_memory() > 0 {
        (system.used_memory() as f32 / system.total_memory() as f32) * 100.0
    } else {
        0.0
    };

    let disks = Disks::new_with_refreshed_list();
    let (total_disk, available_disk) = disks
        .iter()
        .fold((0_u64, 0_u64), |acc, disk| (acc.0 + disk.total_space(), acc.1 + disk.available_space()));
    let disk_percent = if total_disk > 0 {
        ((total_disk - available_disk) as f32 / total_disk as f32) * 100.0
    } else {
        0.0
    };

    let load = System::load_average().one;

    SystemSnapshot {
        server_id,
        hostname: System::host_name().unwrap_or_else(|| "unknown".to_string()),
        os: System::long_os_version().unwrap_or_else(|| "unknown".to_string()),
        kernel: System::kernel_version().unwrap_or_else(|| "unknown".to_string()),
        architecture: std::env::consts::ARCH.to_string(),
        cpu_percent,
        ram_percent,
        disk_percent,
        load,
        uptime_seconds: System::uptime(),
        agent_version,
        captured_at: Utc::now(),
    }
}

pub fn service_statuses(services: &[String]) -> Result<Vec<ServiceState>> {
    let mut out = Vec::with_capacity(services.len());

    for service in services {
        let status = Command::new("systemctl")
            .arg("is-active")
            .arg(service)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        out.push(ServiceState {
            name: service.clone(),
            status,
        });
    }

    Ok(out)
}
