use dscan_core::ScanSession;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgressResponse {
    pub total_bytes: u64,
    pub total_files: u64,
    pub active_workers: usize,
    pub current_path: String,
    pub elapsed_millis: u64,
    pub files_per_sec: f64,
    pub bytes_per_sec: f64,
    pub is_complete: bool,
    pub is_paused: bool,
}

impl From<dscan_core::ScanProgressDto> for ScanProgressResponse {
    fn from(dto: dscan_core::ScanProgressDto) -> Self {
        Self {
            total_bytes: dto.total_bytes,
            total_files: dto.total_files,
            active_workers: dto.active_workers,
            current_path: dto.current_path,
            elapsed_millis: dto.elapsed_millis,
            files_per_sec: dto.files_per_sec,
            bytes_per_sec: dto.bytes_per_sec,
            is_complete: dto.is_complete,
            is_paused: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreemapNodeResponse {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub total_bytes: u64,
    pub direct_bytes: u64,
    pub rel_depth: u16,
    pub is_dir: bool,
    pub extension: String,
    pub children_ids: Vec<u32>,
}

impl From<dscan_core::TreemapNodeDto> for TreemapNodeResponse {
    fn from(dto: dscan_core::TreemapNodeDto) -> Self {
        Self {
            id: dto.id,
            parent_id: dto.parent_id,
            name: dto.name,
            total_bytes: dto.total_bytes,
            direct_bytes: dto.direct_bytes,
            rel_depth: dto.rel_depth,
            is_dir: dto.is_dir,
            extension: dto.extension,
            children_ids: dto.children_ids,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionStatResponse {
    pub extension: String,
    pub total_bytes: u64,
    pub file_count: u64,
    pub percentage_of_total: f64,
}

impl From<dscan_core::ExtensionStatDto> for ExtensionStatResponse {
    fn from(dto: dscan_core::ExtensionStatDto) -> Self {
        Self {
            extension: dto.extension,
            total_bytes: dto.total_bytes,
            file_count: dto.file_count,
            percentage_of_total: dto.percentage_of_total,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveInfo {
    pub path: String,
    pub name: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

pub struct AppState {
    pub session: RwLock<Option<Arc<ScanSession>>>,
    pub current_path: RwLock<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            session: RwLock::new(None),
            current_path: RwLock::new(String::new()),
        }
    }
}
