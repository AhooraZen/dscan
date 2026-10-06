use std::path::{Path, PathBuf};
use sysinfo::Disks;

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
    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = path.parent() {
            open::that(parent).map_err(|e| e.to_string())?;
        } else {
            open::that(path).map_err(|e| e.to_string())?;
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        open::that(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Move file or directory to OS trash/recycle bin safely
pub fn move_to_trash(path: &Path, scan_root: &Path) -> Result<(), String> {
    is_safe_to_trash(path, scan_root).map_err(ToString::to_string)?;
    trash::delete(path).map_err(|e| format!("Failed to move to trash: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

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
