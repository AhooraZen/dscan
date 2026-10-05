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

/// Open path in desktop environment native file manager
pub fn reveal_in_file_manager(path: &Path) -> Result<(), std::io::Error> {
    let target = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    open::that(target).map_err(|e| std::io::Error::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
