use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::process::Command;

use dscan_core::{ScanOptions, ScanSession};
use tauri::State;

use crate::state::{
    AppState, DriveInfo, ExtensionStatResponse, ScanProgressResponse, TreemapNodeResponse,
};

#[tauri::command]
pub fn start_scan(
    path: String,
    threads: Option<usize>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // 1. Cancel previous session if any
    if let Some(ref prev) = *state.session.read() {
        prev.cancel();
    }

    let target_path = if path.trim().is_empty() {
        ".".to_string()
    } else {
        path.trim().to_string()
    };

    let default_threads = std::thread::available_parallelism()
        .map(|n| (n.get() * 2).clamp(4, 64))
        .unwrap_or(16);

    let opts = ScanOptions {
        target_path: target_path.clone(),
        threads: threads.unwrap_or(default_threads),
        collect_ext_stats: true,
        ..Default::default()
    };

    let session = ScanSession::start(opts).map_err(|e| format!("Failed to start scan: {e}"))?;

    *state.session.write() = Some(session);
    *state.current_path.write() = target_path;
    Ok(())
}

#[tauri::command]
pub fn poll_progress(state: State<'_, AppState>) -> Result<ScanProgressResponse, String> {
    let guard = state.session.read();
    if let Some(ref session) = *guard {
        let p = session.poll_progress();
        let mut resp = ScanProgressResponse::from(p);
        resp.is_paused = session.is_paused();
        Ok(resp)
    } else {
        Ok(ScanProgressResponse {
            total_bytes: 0,
            total_files: 0,
            active_workers: 0,
            current_path: String::new(),
            elapsed_millis: 0,
            files_per_sec: 0.0,
            bytes_per_sec: 0.0,
            is_complete: false,
            is_paused: false,
        })
    }
}

#[tauri::command]
pub fn get_treemap_data(
    node_id: Option<u32>,
    depth: Option<u16>,
    max_nodes: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<TreemapNodeResponse>, String> {
    let _ = node_id;
    let guard = state.session.read();
    if let Some(ref session) = *guard {
        let max_d = depth.unwrap_or(6);
        let max_n = max_nodes.unwrap_or(5000);
        let nodes = session.get_hierarchical_view(max_d, max_n);
        Ok(nodes.into_iter().map(TreemapNodeResponse::from).collect())
    } else {
        Ok(Vec::new())
    }
}

#[tauri::command]
pub fn get_extension_legend(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<ExtensionStatResponse>, String> {
    let guard = state.session.read();
    if let Some(ref session) = *guard {
        let lim = limit.unwrap_or(24);
        let exts = session.get_extension_breakdown(lim);
        Ok(exts.into_iter().map(ExtensionStatResponse::from).collect())
    } else {
        Ok(Vec::new())
    }
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.session.read();
    if let Some(ref session) = *guard {
        session.cancel();
    }
    Ok(())
}

#[tauri::command]
pub fn pause_scan(state: State<'_, AppState>) -> Result<bool, String> {
    let guard = state.session.read();
    if let Some(ref session) = *guard {
        if session.is_paused() {
            session.resume();
            Ok(false)
        } else {
            session.pause();
            Ok(true)
        }
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub fn open_in_file_manager(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    let _target_dir = if p.is_dir() {
        p
    } else {
        p.parent().unwrap_or(Path::new("/"))
    };

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(_target_dir)
            .spawn()
            .map_err(|e| format!("Failed to open file manager: {e}"))?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(_target_dir)
            .spawn()
            .map_err(|e| format!("Failed to open explorer: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(target_dir)
            .spawn()
            .map_err(|e| format!("Failed to open finder: {e}"))?;
    }

    Ok(())
}

#[tauri::command]
pub fn get_system_drives() -> Result<Vec<DriveInfo>, String> {
    let mut drives = Vec::new();

    #[cfg(unix)]
    {
        // Add common roots
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
        drives.push(DriveInfo {
            path: home.clone(),
            name: format!("Home ({home})"),
            total_bytes: 1_000_000_000_000,
            free_bytes: 500_000_000_000,
        });

        drives.push(DriveInfo {
            path: "/".to_string(),
            name: "Root Filesystem (/)".to_string(),
            total_bytes: 1_000_000_000_000,
            free_bytes: 500_000_000_000,
        });

        // Scan /proc/mounts if available
        if let Ok(content) = std::fs::read_to_string("/proc/mounts") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let dev = parts[0];
                    let mount = parts[1];
                    let fstype = parts[2];
                    if (dev.starts_with("/dev/") || dev.starts_with("/dev/mapper/"))
                        && !mount.starts_with("/boot")
                        && !mount.starts_with("/var/lib/docker")
                        && mount != "/"
                        && mount != home
                    {
                        drives.push(DriveInfo {
                            path: mount.to_string(),
                            name: format!("{mount} ({fstype})"),
                            total_bytes: 0,
                            free_bytes: 0,
                        });
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    {
        for letter in b'A'..=b'Z' {
            let root = format!("{}:\\", letter as char);
            if Path::new(&root).exists() {
                drives.push(DriveInfo {
                    path: root.clone(),
                    name: format!("Local Disk ({})", root),
                    total_bytes: 0,
                    free_bytes: 0,
                });
            }
        }
    }

    Ok(drives)
}
