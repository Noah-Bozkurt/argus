use anyhow::Result;
use chrono::Utc;
use protocol::{
    BackupState, DiagnosticsState, DockerState, MountState, NetworkInterfaceState, PackageUpdate,
    ProcessState, SecurityState, ServiceState, SystemSnapshot, UpdateState,
};
use std::{cmp::Ordering, collections::BTreeSet, path::Path, process::Command};
use sysinfo::{Disks, Networks, System};
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
    let mounts = disks
        .iter()
        .map(|disk| MountState {
            name: disk.name().to_string_lossy().into_owned(),
            mount_point: disk.mount_point().to_string_lossy().into_owned(),
            file_system: disk.file_system().to_string_lossy().into_owned(),
            total_bytes: disk.total_space(),
            available_bytes: disk.available_space(),
        })
        .collect();
    let networks = Networks::new_with_refreshed_list();
    let network = networks
        .iter()
        .map(|(name, data)| NetworkInterfaceState {
            name: name.clone(),
            received_bytes: data.total_received(),
            transmitted_bytes: data.total_transmitted(),
            receive_errors: data.total_errors_on_received(),
            transmit_errors: data.total_errors_on_transmitted(),
        })
        .collect();
    let mut top_processes = system
        .processes()
        .iter()
        .map(|(pid, process)| ProcessState {
            pid: pid.as_u32(),
            name: process.name().to_string_lossy().into_owned(),
            cpu_percent: process.cpu_usage(),
            memory_bytes: process.memory(),
        })
        .collect::<Vec<_>>();
    top_processes.sort_by(|a, b| {
        b.cpu_percent
            .partial_cmp(&a.cpu_percent)
            .unwrap_or(Ordering::Equal)
            .then_with(|| b.memory_bytes.cmp(&a.memory_bytes))
    });
    top_processes.truncate(25);
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
        docker: DockerState::default(),
        security: SecurityState::default(),
        backups: BackupState::default(),
        mounts,
        network,
        top_processes,
        captured_at: Utc::now(),
    }
}

pub fn update_state() -> UpdateState {
    let reboot_required = Path::new("/var/run/reboot-required").exists();
    let output = Command::new("apt-get")
        .args(["-s", "-o", "Debug::NoLocking=1", "upgrade"])
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let packages = parse_apt_updates(&text);
            UpdateState {
                supported: true,
                pending_updates: packages.len().try_into().unwrap_or(u32::MAX),
                reboot_required,
                packages,
            }
        }
        _ => UpdateState {
            supported: false,
            pending_updates: 0,
            reboot_required,
            packages: Vec::new(),
        },
    }
}
fn parse_apt_updates(output: &str) -> Vec<PackageUpdate> {
    output
        .lines()
        .filter(|line| line.starts_with("Inst "))
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            parts.next()?;
            let name = parts.next()?.to_string();
            let installed_version = line
                .split('[')
                .nth(1)
                .and_then(|value| value.split(']').next())
                .unwrap_or_default()
                .to_string();
            let candidate_version = line
                .split('(')
                .nth(1)
                .and_then(|value| value.split_whitespace().next())
                .unwrap_or_default()
                .trim_end_matches(')')
                .to_string();
            let security = line.to_ascii_lowercase().contains("security");
            Some(PackageUpdate {
                name,
                installed_version,
                candidate_version,
                security,
            })
        })
        .take(500)
        .collect()
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
        assert_eq!(parse_apt_updates(output).len(), 2);
    }
    #[test]
    fn apt_inventory_preserves_versions_and_security_classification() {
        let updates = parse_apt_updates("Inst openssl [3.0.1] (3.0.2 Ubuntu:security)\n");
        assert_eq!(updates[0].name, "openssl");
        assert_eq!(updates[0].installed_version, "3.0.1");
        assert_eq!(updates[0].candidate_version, "3.0.2");
        assert!(updates[0].security);
    }
}
