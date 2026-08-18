use anyhow::Result;
use chrono::Utc;
use protocol::{DiagnosticsState, ServiceState, SystemSnapshot, UpdateState};
use std::{collections::BTreeSet, path::Path, process::Command};
use sysinfo::{Disks, System};
use uuid::Uuid;

pub fn current_uptime_seconds() -> u64 {
    System::uptime()
}

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
    let (total_disk, available_disk) = disks.iter().fold((0_u64, 0_u64), |acc, disk| {
        (acc.0 + disk.total_space(), acc.1 + disk.available_space())
    });
    let disk_percent = if total_disk > 0 {
        ((total_disk - available_disk) as f32 / total_disk as f32) * 100.0
    } else {
        0.0
    };

    SystemSnapshot {
        server_id,
        hostname: System::host_name().unwrap_or_else(|| "unknown".to_string()),
        os: System::long_os_version().unwrap_or_else(|| "unknown".to_string()),
        kernel: System::kernel_version().unwrap_or_else(|| "unknown".to_string()),
        architecture: std::env::consts::ARCH.to_string(),
        cpu_percent,
        ram_percent,
        disk_percent,
        load: System::load_average().one,
        uptime_seconds: System::uptime(),
        agent_version,
        updates: UpdateState::default(),
        diagnostics: DiagnosticsState::default(),
        captured_at: Utc::now(),
    }
}

pub fn update_state() -> UpdateState {
    let reboot_required = Path::new("/var/run/reboot-required").exists();
    let output = Command::new("apt-get")
        .args(["-s", "-o", "Debug::NoLocking=1", "upgrade"])
        .output();

    match output {
        Ok(output) if output.status.success() => UpdateState {
            supported: true,
            pending_updates: count_simulated_apt_updates(&String::from_utf8_lossy(&output.stdout)),
            reboot_required,
        },
        _ => UpdateState {
            supported: false,
            pending_updates: 0,
            reboot_required,
        },
    }
}

pub fn diagnostics_state() -> DiagnosticsState {
    DiagnosticsState {
        failed_units: failed_units(),
        listening_tcp_ports: listening_tcp_ports(),
        journals: Vec::new(),
    }
}

fn failed_units() -> Vec<String> {
    Command::new("systemctl")
        .args(["--failed", "--no-legend", "--plain"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| line.split_whitespace().next())
                .filter(|unit| unit.ends_with(".service"))
                .take(50)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn listening_tcp_ports() -> Vec<u16> {
    let output = Command::new("ss").args(["-ltnH"]).output();
    let mut ports = BTreeSet::new();
    if let Ok(output) = output {
        if output.status.success() {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                if let Some(local) = line.split_whitespace().nth(3) {
                    if let Some(port) = local
                        .rsplit(':')
                        .next()
                        .and_then(|value| value.parse().ok())
                    {
                        ports.insert(port);
                    }
                }
            }
        }
    }
    ports.into_iter().take(200).collect()
}

fn count_simulated_apt_updates(output: &str) -> u32 {
    output
        .lines()
        .filter(|line| line.starts_with("Inst "))
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_simulated_install_lines() {
        let output = "Reading package lists...\nInst openssl [1.0] (1.1 Ubuntu:stable)\nConf openssl (1.1 Ubuntu:stable)\nInst curl [8.0] (8.1 Ubuntu:stable)\n";
        assert_eq!(count_simulated_apt_updates(output), 2);
    }
}
