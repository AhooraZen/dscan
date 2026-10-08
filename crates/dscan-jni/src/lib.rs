use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dscan_core::ScanOptions;
use dscan_core::snapshot::ScanSession;
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{JNI_FALSE, JNI_TRUE, jboolean, jint, jlong, jstring};
use sysinfo::Disks;

fn escape_json_str(s: &str, out: &mut String) {
    out.reserve(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\x08' => out.push_str("\\b"),
            '\x0C' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveInfo {
    pub name: String,
    pub mount_point: PathBuf,
    pub total_space: u64,
    pub available_space: u64,
    pub file_system: String,
}

impl DriveInfo {
    pub fn used_space(&self) -> u64 {
        self.total_space.saturating_sub(self.available_space)
    }

    pub fn used_percentage(&self) -> f64 {
        if self.total_space == 0 {
            0.0
        } else {
            (self.used_space() as f64 / self.total_space as f64) * 100.0
        }
    }
}

/// Query system for mounted logical drives and partitions
pub fn detect_drives() -> Vec<DriveInfo> {
    let disks = Disks::new_with_refreshed_list();
    let mut drives = Vec::new();
    for disk in disks.list() {
        let name_str = disk.name().to_string_lossy().to_string();
        let fs_str = disk.file_system().to_string_lossy().to_string();
        let mount_point = disk.mount_point().to_path_buf();
        drives.push(DriveInfo {
            name: if name_str.is_empty() {
                mount_point.to_string_lossy().to_string()
            } else {
                name_str
            },
            mount_point,
            total_space: disk.total_space(),
            available_space: disk.available_space(),
            file_system: fs_str,
        });
    }
    drives.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    drives
}

/// Safety guard preventing accidental deletion of critical system files or roots
pub fn is_safe_to_trash(path: &Path, scan_root: &Path) -> Result<(), &'static str> {
    // Disallow root ("/", "C:\")
    if path.parent().is_none() {
        return Err("Cannot trash filesystem root");
    }
    // Disallow scan root itself
    if path == scan_root {
        return Err("Cannot trash scan root directory");
    }
    // Disallow home root directory
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
        && path == Path::new(&home)
    {
        return Err("Cannot trash user home directory");
    }
    // Disallow essential system directories
    let p_str = path.to_string_lossy();
    let p_lower = p_str.to_lowercase();
    let forbidden = [
        "/bin",
        "/sbin",
        "/usr",
        "/etc",
        "/boot",
        "/dev",
        "/proc",
        "/sys",
        "c:\\windows",
        "c:/windows",
        "c:\\program files",
        "c:/program files",
        "c:\\program files (x86)",
        "c:/program files (x86)",
    ];
    for f in forbidden {
        if p_lower.starts_with(f)
            && (p_lower.len() == f.len()
                || p_lower.as_bytes()[f.len()] == b'/'
                || p_lower.as_bytes()[f.len()] == b'\\')
        {
            return Err("Cannot trash critical operating system path");
        }
    }
    Ok(())
}

/// Open path in desktop environment native file manager
pub fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt as _;
        let p = path.to_string_lossy();
        std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{p}\""))
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path.to_string_lossy()])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(all(target_os = "linux", not(target_os = "android")))]
    {
        if let Some(parent) = path.parent() {
            open::that(parent).map_err(|e| e.to_string())?;
        } else {
            open::that(path).map_err(|e| e.to_string())?;
        }
    }
    #[cfg(target_os = "android")]
    {
        let _ = path;
        return Err("Reveal in file manager is not supported on Android".to_string());
    }
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux",
        target_os = "android"
    )))]
    {
        open::that(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Move file or directory to OS trash/recycle bin safely
pub fn move_to_trash(path: &Path, scan_root: &Path) -> Result<(), String> {
    is_safe_to_trash(path, scan_root).map_err(ToString::to_string)?;
    #[cfg(not(target_os = "android"))]
    {
        trash::delete(path).map_err(|e| format!("Failed to move to trash: {e}"))
    }
    #[cfg(target_os = "android")]
    {
        let _ = path;
        Err("Moving to trash is not supported on Android".to_string())
    }
}

/// Start an asynchronous disk scan session.
/// Returns a raw pointer cast to jlong, or 0 on error.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_startScan(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
    threads: jint,
) -> jlong {
    let path_str: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };

    let p = std::path::Path::new(&path_str);
    let auto_threads = if threads > 0 {
        threads as usize
    } else {
        ScanOptions::auto_threads_for_path(p)
    };

    let opts = ScanOptions {
        target_path: path_str,
        top_limit: 500,
        max_depth: usize::MAX,
        threads: auto_threads,
        excludes: vec![
            "/proc".to_string(),
            "/sys".to_string(),
            "/dev".to_string(),
            ".git".to_string(),
        ],
        follow_symlinks: false,
        cross_filesystems: false,
        collect_ext_stats: true,
    };

    match ScanSession::start(opts) {
        Ok(session) => {
            let boxed = Box::new(session);
            Box::into_raw(boxed) as jlong
        }
        Err(_) => 0,
    }
}

/// Poll the scan progress of an active session.
/// Returns a JSON string with progress metrics.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_pollProgress(
    env: JNIEnv,
    _class: JClass,
    session_ptr: jlong,
) -> jstring {
    if session_ptr == 0 {
        return env
            .new_string("{}")
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut());
    }

    let session: &Arc<ScanSession> = unsafe { &*(session_ptr as *const Arc<ScanSession>) };
    let prog = session.poll_progress();

    let mut json = String::with_capacity(512);
    json.push_str("{\"total_bytes\":");
    let _ = write!(json, "{}", prog.total_bytes);
    json.push_str(",\"total_files\":");
    let _ = write!(json, "{}", prog.total_files);
    json.push_str(",\"active_workers\":");
    let _ = write!(json, "{}", prog.active_workers);
    json.push_str(",\"elapsed_millis\":");
    let _ = write!(json, "{}", prog.elapsed_millis);
    json.push_str(",\"files_per_sec\":");
    let _ = write!(json, "{:.2}", prog.files_per_sec);
    json.push_str(",\"bytes_per_sec\":");
    let _ = write!(json, "{:.2}", prog.bytes_per_sec);
    json.push_str(",\"is_complete\":");
    json.push_str(if prog.is_complete { "true" } else { "false" });
    json.push_str(",\"current_path\":\"");
    escape_json_str(&prog.current_path, &mut json);
    json.push_str("\"}");

    env.new_string(&json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Retrieve hierarchical treemap nodes as JSON.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_getTreemapNodes(
    env: JNIEnv,
    _class: JClass,
    session_ptr: jlong,
    max_depth: jint,
    max_nodes: jint,
) -> jstring {
    if session_ptr == 0 {
        return env
            .new_string("[]")
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut());
    }

    let session: &Arc<ScanSession> = unsafe { &*(session_ptr as *const Arc<ScanSession>) };
    let depth = if max_depth > 0 { max_depth as u16 } else { 6 };
    let count = if max_nodes > 0 {
        max_nodes as usize
    } else {
        2048
    };

    let nodes = session.get_hierarchical_view(depth, count);

    let mut json = String::with_capacity(nodes.len() * 128 + 16);
    json.push('[');

    for (i, node) in nodes.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str("{\"id\":");
        let _ = write!(json, "{}", node.id);
        json.push_str(",\"parent_id\":");
        let _ = write!(json, "{}", node.parent_id);
        json.push_str(",\"name\":\"");
        escape_json_str(&node.name, &mut json);
        json.push_str("\",\"total_bytes\":");
        let _ = write!(json, "{}", node.total_bytes);
        json.push_str(",\"direct_bytes\":");
        let _ = write!(json, "{}", node.direct_bytes);
        json.push_str(",\"rel_depth\":");
        let _ = write!(json, "{}", node.rel_depth);
        json.push_str(",\"is_dir\":");
        json.push_str(if node.is_dir { "true" } else { "false" });
        json.push_str(",\"extension\":\"");
        escape_json_str(&node.extension, &mut json);
        json.push_str("\",\"children_ids\":[");
        for (ci, &cid) in node.children_ids.iter().enumerate() {
            if ci > 0 {
                json.push(',');
            }
            let _ = write!(json, "{}", cid);
        }
        json.push_str("]}");
    }

    json.push(']');

    env.new_string(&json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Retrieve file extension breakdown statistics as JSON.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_getExtensionBreakdown(
    env: JNIEnv,
    _class: JClass,
    session_ptr: jlong,
    limit: jint,
) -> jstring {
    if session_ptr == 0 {
        return env
            .new_string("[]")
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut());
    }

    let session: &Arc<ScanSession> = unsafe { &*(session_ptr as *const Arc<ScanSession>) };
    let lim = if limit > 0 { limit as usize } else { 16 };
    let exts = session.get_extension_breakdown(lim);

    let mut json = String::with_capacity(exts.len() * 96 + 16);
    json.push('[');

    for (i, ext) in exts.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str("{\"extension\":\"");
        escape_json_str(&ext.extension, &mut json);
        json.push_str("\",\"total_bytes\":");
        let _ = write!(json, "{}", ext.total_bytes);
        json.push_str(",\"file_count\":");
        let _ = write!(json, "{}", ext.file_count);
        json.push_str(",\"percentage\":");
        let _ = write!(json, "{:.2}", ext.percentage_of_total);
        json.push('}');
    }

    json.push(']');

    env.new_string(&json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Request cancellation of the active scan.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_cancelScan(
    _env: JNIEnv,
    _class: JClass,
    session_ptr: jlong,
) {
    if session_ptr == 0 {
        return;
    }
    let session: &Arc<ScanSession> = unsafe { &*(session_ptr as *const Arc<ScanSession>) };
    session.cancel();
}

/// Stop scan and safely free the session Arc pointer.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_stopScan(
    _env: JNIEnv,
    _class: JClass,
    session_ptr: jlong,
) {
    if session_ptr == 0 {
        return;
    }
    let boxed = unsafe { Box::from_raw(session_ptr as *mut Arc<ScanSession>) };
    boxed.cancel();
    drop(boxed);
}

/// Query system for mounted logical drives and partitions as JSON.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_detectDrives(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let drives = detect_drives();
    let mut json = String::with_capacity(drives.len() * 128 + 16);
    json.push('[');
    for (i, d) in drives.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str("{\"name\":\"");
        escape_json_str(&d.name, &mut json);
        json.push_str("\",\"mount_point\":\"");
        escape_json_str(&d.mount_point.to_string_lossy(), &mut json);
        json.push_str("\",\"total_space\":");
        let _ = write!(json, "{}", d.total_space);
        json.push_str(",\"available_space\":");
        let _ = write!(json, "{}", d.available_space);
        json.push_str(",\"file_system\":\"");
        escape_json_str(&d.file_system, &mut json);
        json.push_str("\"}");
    }
    json.push(']');

    env.new_string(&json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Open path in desktop environment native file manager.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_revealInFileManager(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
) -> jboolean {
    let path_str: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };
    let p = Path::new(&path_str);
    if reveal_in_file_manager(p).is_ok() {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

/// Move file or directory to OS trash/recycle bin safely.
/// Returns null on success, or error string on failure.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_moveToTrash(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
    root_path: JString,
) -> jstring {
    let path_str: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(e) => {
            return env
                .new_string(format!("Failed to read path: {e}"))
                .map(|s| s.into_raw())
                .unwrap_or(std::ptr::null_mut());
        }
    };
    let root_str: String = match env.get_string(&root_path) {
        Ok(s) => s.into(),
        Err(e) => {
            return env
                .new_string(format!("Failed to read root path: {e}"))
                .map(|s| s.into_raw())
                .unwrap_or(std::ptr::null_mut());
        }
    };

    let p = Path::new(&path_str);
    let r = Path::new(&root_str);

    match move_to_trash(p, r) {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => env
            .new_string(&e)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_json_str() {
        let mut out = String::new();
        escape_json_str("hello \"world\"\n\t\\", &mut out);
        assert_eq!(out, "hello \\\"world\\\"\\n\\t\\\\");
    }

    #[test]
    fn test_is_safe_to_trash_guards() {
        let scan_root = Path::new("/home/user/workspace");

        // Root cannot be trashed
        assert!(is_safe_to_trash(Path::new("/"), scan_root).is_err());

        // Scan root itself cannot be trashed
        assert!(is_safe_to_trash(scan_root, scan_root).is_err());

        // System directories cannot be trashed
        assert!(is_safe_to_trash(Path::new("/usr/bin/python3"), scan_root).is_err());
        assert!(is_safe_to_trash(Path::new("/etc/passwd"), scan_root).is_err());
        assert!(is_safe_to_trash(Path::new("/bin/ls"), scan_root).is_err());
        assert!(is_safe_to_trash(Path::new("C:\\Windows\\System32"), scan_root).is_err());

        // Normal path inside scan root is safe
        let safe_file = Path::new("/home/user/workspace/target/debug/build.log");
        assert!(is_safe_to_trash(safe_file, scan_root).is_ok());
    }

    #[test]
    fn test_detect_drives_non_empty() {
        let drives = detect_drives();
        assert!(!drives.is_empty());
        for drive in &drives {
            assert!(!drive.mount_point.as_os_str().is_empty());
        }
    }

    #[test]
    fn test_drive_info_used_space() {
        let drive = DriveInfo {
            name: "test".to_string(),
            mount_point: PathBuf::from("/test"),
            total_space: 1000,
            available_space: 400,
            file_system: "ext4".to_string(),
        };
        assert_eq!(drive.used_space(), 600);
        assert!((drive.used_percentage() - 60.0).abs() < 1e-4);
    }
}
