use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dscan_core::scanner::ScanOptions;
use dscan_core::snapshot::{ExtensionStatDto, ScanProgressDto, ScanSession, TreemapNodeDto};

use crate::system::{DriveInfo, detect_drives};
use crate::treemap::squarify::{LaidOutTreemapNode, Rect, build_hierarchical_layout};

pub struct AppState {
    pub target_path: PathBuf,
    pub drives: Vec<DriveInfo>,
    pub selected_drive_idx: usize,
    pub session: Option<Arc<ScanSession>>,
    pub progress: Option<ScanProgressDto>,
    pub raw_nodes: Vec<TreemapNodeDto>,
    pub layout_nodes: Vec<LaidOutTreemapNode>,
    pub extensions: Vec<ExtensionStatDto>,
    pub selected_node_id: Option<u32>,
    pub hovered_node_id: Option<u32>,
    pub selected_extension: Option<String>,
    pub expanded_dirs: HashSet<u32>,
    pub is_scanning: bool,
    pub is_paused: bool,
    pub is_complete: bool,
    pub layout_width: f32,
    pub layout_height: f32,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        let drives = detect_drives();
        let target_path = drives
            .first()
            .map(|d| d.mount_point.clone())
            .unwrap_or_else(|| PathBuf::from("/"));

        Self {
            target_path,
            drives,
            selected_drive_idx: 0,
            session: None,
            progress: None,
            raw_nodes: Vec::new(),
            layout_nodes: Vec::new(),
            extensions: Vec::new(),
            selected_node_id: None,
            hovered_node_id: None,
            selected_extension: None,
            expanded_dirs: HashSet::new(),
            is_scanning: false,
            is_paused: false,
            is_complete: false,
            layout_width: 800.0,
            layout_height: 500.0,
        }
    }

    /// Select logical drive by index and update target path
    pub fn select_drive(&mut self, idx: usize) {
        if idx < self.drives.len() {
            self.selected_drive_idx = idx;
            self.target_path = self.drives[idx].mount_point.clone();
        }
    }

    /// Start or restart scanning target path
    pub fn start_scan(&mut self, path: &Path, threads: usize) -> Result<(), std::io::Error> {
        self.target_path = path.to_path_buf();
        let opts = ScanOptions {
            target_path: path.to_string_lossy().to_string(),
            top_limit: 500,
            max_depth: 8,
            threads: threads.max(1),
            excludes: vec![],
            follow_symlinks: false,
            cross_filesystems: false,
            collect_ext_stats: true,
        };

        let session = ScanSession::start(opts)?;
        self.session = Some(session);
        self.progress = None;
        self.raw_nodes.clear();
        self.layout_nodes.clear();
        self.extensions.clear();
        self.selected_node_id = None;
        self.hovered_node_id = None;
        self.selected_extension = None;
        self.expanded_dirs.clear();
        self.expanded_dirs.insert(0); // Root is expanded by default
        self.is_scanning = true;
        self.is_paused = false;
        self.is_complete = false;

        Ok(())
    }

    /// Pause active scan
    pub fn pause_scan(&mut self) {
        if let Some(ref session) = self.session {
            session.pause();
            self.is_paused = true;
        }
    }

    /// Resume paused scan
    pub fn resume_scan(&mut self) {
        if let Some(ref session) = self.session {
            session.resume();
            self.is_paused = false;
        }
    }

    /// Cancel active scan
    pub fn cancel_scan(&mut self) {
        if let Some(ref session) = self.session {
            session.cancel();
            self.is_scanning = false;
            self.is_paused = false;
        }
    }

    /// Poll session progress, update stats, build final treemap once complete
    pub fn poll_progress(&mut self) {
        let Some(ref session) = self.session else {
            return;
        };

        let prog = session.poll_progress();
        self.progress = Some(prog.clone());

        if prog.is_complete && !self.is_complete {
            self.is_complete = true;
            self.is_scanning = false;
            self.is_paused = false;

            // Retrieve hierarchical view up to depth 6 and 2048 nodes
            self.raw_nodes = session.get_hierarchical_view(6, 2048);
            self.extensions = session.get_extension_breakdown(16);

            self.rebuild_layout();
        }
    }

    /// Recompute squarified cushion treemap layout
    pub fn rebuild_layout(&mut self) {
        if self.raw_nodes.is_empty() {
            self.layout_nodes.clear();
            return;
        }
        let bounds = Rect::new(0.0, 0.0, self.layout_width, self.layout_height);
        self.layout_nodes = build_hierarchical_layout(&self.raw_nodes, bounds);
    }

    /// Resize canvas dimensions and recompute layout if changed
    pub fn update_layout_size(&mut self, width: f32, height: f32) {
        let w = width.max(10.0);
        let h = height.max(10.0);
        if (self.layout_width - w).abs() > 1.0 || (self.layout_height - h).abs() > 1.0 {
            self.layout_width = w;
            self.layout_height = h;
            if !self.raw_nodes.is_empty() {
                self.rebuild_layout();
            }
        }
    }

    /// Find node id at local canvas coordinate (px, py)
    pub fn hit_test(&self, px: f32, py: f32) -> Option<u32> {
        // Search leaf nodes first, reverse order (topmost)
        for node in self.layout_nodes.iter().rev() {
            if !node.is_dir && node.rect.contains(px, py) {
                return Some(node.id);
            }
        }
        // Fallback to directory containers
        for node in self.layout_nodes.iter().rev() {
            if node.rect.contains(px, py) {
                return Some(node.id);
            }
        }
        None
    }

    /// Lookup raw node by id
    pub fn find_node(&self, id: u32) -> Option<&TreemapNodeDto> {
        self.raw_nodes.get(id as usize)
    }

    /// Toggle expansion of directory node in tree table
    pub fn toggle_dir_expanded(&mut self, dir_id: u32) {
        if self.expanded_dirs.contains(&dir_id) {
            self.expanded_dirs.remove(&dir_id);
        } else {
            self.expanded_dirs.insert(dir_id);
        }
    }

    /// Toggle extension filter highlight
    pub fn toggle_extension_filter(&mut self, ext: &str) {
        if self.selected_extension.as_deref() == Some(ext) {
            self.selected_extension = None;
        } else {
            self.selected_extension = Some(ext.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_defaults() {
        let state = AppState::new();
        assert!(!state.is_scanning);
        assert!(!state.is_complete);
        assert_eq!(state.layout_width, 800.0);
        assert_eq!(state.layout_height, 500.0);
    }

    #[test]
    fn test_toggle_dir_and_ext() {
        let mut state = AppState::new();
        state.toggle_dir_expanded(5);
        assert!(state.expanded_dirs.contains(&5));
        state.toggle_dir_expanded(5);
        assert!(!state.expanded_dirs.contains(&5));

        state.toggle_extension_filter(".rs");
        assert_eq!(state.selected_extension.as_deref(), Some(".rs"));
        state.toggle_extension_filter(".rs");
        assert_eq!(state.selected_extension, None);
    }
}
