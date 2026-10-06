use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dscan_core::scanner::ScanOptions;
use dscan_core::snapshot::{
    ExtensionStatDto, ScanProgressDto, ScanSession, TreemapNodeDto, build_extension_breakdown,
};

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
    pub treemap_origin_y: f32,
    pub theme_mode: crate::theme::ThemeMode,
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
            .unwrap_or_else(|| {
                if cfg!(windows) {
                    PathBuf::from("C:\\")
                } else {
                    PathBuf::from("/")
                }
            });

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
            treemap_origin_y: 0.0,
            theme_mode: crate::theme::ThemeMode::Dark,
        }
    }

    /// Toggle between Dark and Light mode
    pub fn toggle_theme(&mut self) {
        self.theme_mode = self.theme_mode.toggle();
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
            top_limit: 5000,
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

            // Retrieve hierarchical view up to depth 8 and 8192 nodes
            self.raw_nodes = session.get_hierarchical_view(8, 8192);
            self.extensions = session.get_extension_breakdown(24);

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

    /// Find node id at window coordinate (window_px, window_py)
    pub fn hit_test(&self, window_px: f32, window_py: f32) -> Option<u32> {
        let px = window_px;
        let py = window_py - self.treemap_origin_y;
        if py < 0.0 || py > self.layout_height || px < 0.0 || px > self.layout_width {
            return None;
        }
        // Search leaf nodes first, reverse order (topmost)
        for node in self.layout_nodes.iter().rev() {
            if !node.has_children && node.rect.contains(px, py) {
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

    /// Reconstruct filesystem path for a node by traversing ancestor hierarchy
    pub fn node_full_path(&self, id: u32) -> PathBuf {
        if id == 0 {
            return self.target_path.clone();
        }
        let mut segments = Vec::new();
        let mut curr_id = id;
        while curr_id != 0 {
            if let Some(node) = self.find_node(curr_id) {
                segments.push(node.name.clone());
                if node.parent_id == curr_id {
                    break;
                }
                curr_id = node.parent_id;
            } else {
                break;
            }
        }
        segments.reverse();
        let mut full = self.target_path.clone();
        for seg in segments {
            full.push(seg);
        }
        full
    }

    /// Prune node and all its descendants from in-memory state, subtracting size from ancestors
    pub fn prune_node_and_bubble_size(&mut self, node_id: usize) {
        if node_id >= self.raw_nodes.len() {
            return;
        }

        if node_id == 0 {
            self.raw_nodes.clear();
            self.layout_nodes.clear();
            self.extensions.clear();
            self.selected_node_id = None;
            self.hovered_node_id = None;
            self.expanded_dirs.clear();
            return;
        }

        let target_u32 = node_id as u32;
        let pruned_bytes = self.raw_nodes[node_id].total_bytes;

        // 1. Subtract pruned_bytes from ancestors
        let mut curr_parent = self.raw_nodes[node_id].parent_id;
        let mut visited = HashSet::new();
        while (curr_parent as usize) < self.raw_nodes.len() && visited.insert(curr_parent) {
            self.raw_nodes[curr_parent as usize].total_bytes = self.raw_nodes[curr_parent as usize]
                .total_bytes
                .saturating_sub(pruned_bytes);
            if curr_parent == 0 {
                break;
            }
            curr_parent = self.raw_nodes[curr_parent as usize].parent_id;
        }

        // 2. Collect all descendant IDs
        let mut to_remove = HashSet::new();
        let mut stack = vec![target_u32];
        while let Some(curr) = stack.pop() {
            if to_remove.insert(curr)
                && let Some(node) = self.raw_nodes.get(curr as usize)
            {
                for &cid in &node.children_ids {
                    stack.push(cid);
                }
            }
        }

        // 3. Remove target from parent's children_ids
        let parent_id = self.raw_nodes[node_id].parent_id;
        if let Some(parent_node) = self.raw_nodes.get_mut(parent_id as usize) {
            parent_node.children_ids.retain(|&cid| cid != target_u32);
        }

        // 4. Rebuild raw_nodes with updated contiguous IDs
        let mut old_to_new = HashMap::new();
        let mut new_nodes =
            Vec::with_capacity(self.raw_nodes.len().saturating_sub(to_remove.len()));
        for (old_idx, node) in self.raw_nodes.iter().enumerate() {
            let old_id = old_idx as u32;
            if !to_remove.contains(&old_id) {
                let new_id = new_nodes.len() as u32;
                old_to_new.insert(old_id, new_id);
                new_nodes.push(node.clone());
            }
        }

        for node in &mut new_nodes {
            node.id = *old_to_new.get(&node.id).unwrap_or(&0);
            node.parent_id = *old_to_new.get(&node.parent_id).unwrap_or(&0);
            node.children_ids = node
                .children_ids
                .iter()
                .filter_map(|cid| old_to_new.get(cid).copied())
                .collect();
        }
        self.raw_nodes = new_nodes;

        // 5. Update state pointers
        self.expanded_dirs = self
            .expanded_dirs
            .iter()
            .filter_map(|id| old_to_new.get(id).copied())
            .collect();
        self.selected_node_id = self
            .selected_node_id
            .and_then(|id| old_to_new.get(&id).copied());
        self.hovered_node_id = self
            .hovered_node_id
            .and_then(|id| old_to_new.get(&id).copied());

        // 6. Rebuild squarified layout
        self.rebuild_layout();

        // 7. Recompute extension breakdown
        let mut ext_map: HashMap<String, (u64, u64)> = HashMap::new();
        for node in &self.raw_nodes {
            if !node.is_dir && !node.extension.is_empty() {
                let entry = ext_map.entry(node.extension.clone()).or_insert((0, 0));
                entry.0 += node.total_bytes;
                entry.1 += 1;
            }
        }
        let root_total = self.raw_nodes.first().map(|n| n.total_bytes).unwrap_or(0);
        self.extensions = build_extension_breakdown(&ext_map, root_total, 16);
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

    #[test]
    fn test_prune_node_and_bubble_size() {
        let mut state = AppState::new();
        state.raw_nodes = vec![
            TreemapNodeDto {
                id: 0,
                parent_id: 0,
                name: "root".to_string(),
                total_bytes: 1000,
                direct_bytes: 0,
                rel_depth: 0,
                is_dir: true,
                extension: String::new(),
                children_ids: vec![1, 4],
            },
            TreemapNodeDto {
                id: 1,
                parent_id: 0,
                name: "src".to_string(),
                total_bytes: 600,
                direct_bytes: 0,
                rel_depth: 1,
                is_dir: true,
                extension: String::new(),
                children_ids: vec![2, 3],
            },
            TreemapNodeDto {
                id: 2,
                parent_id: 1,
                name: "main.rs".to_string(),
                total_bytes: 400,
                direct_bytes: 400,
                rel_depth: 2,
                is_dir: false,
                extension: ".rs".to_string(),
                children_ids: vec![],
            },
            TreemapNodeDto {
                id: 3,
                parent_id: 1,
                name: "lib.rs".to_string(),
                total_bytes: 200,
                direct_bytes: 200,
                rel_depth: 2,
                is_dir: false,
                extension: ".rs".to_string(),
                children_ids: vec![],
            },
            TreemapNodeDto {
                id: 4,
                parent_id: 0,
                name: "Cargo.toml".to_string(),
                total_bytes: 400,
                direct_bytes: 400,
                rel_depth: 1,
                is_dir: false,
                extension: ".toml".to_string(),
                children_ids: vec![],
            },
        ];
        state.selected_node_id = Some(2);

        // Prune main.rs (node 2, 400 bytes)
        state.prune_node_and_bubble_size(2);

        // Check sizes bubbled up
        assert_eq!(state.raw_nodes[0].total_bytes, 600); // Root: 1000 - 400 = 600
        assert_eq!(state.raw_nodes[1].total_bytes, 200); // src: 600 - 400 = 200

        // Remaining nodes count: 4
        assert_eq!(state.raw_nodes.len(), 4);
        // Node 2 was selected and pruned, so selected_node_id should now be None
        assert_eq!(state.selected_node_id, None);

        // Check extensions: .rs should now be 200 bytes, .toml 400 bytes
        let rs_stat = state.extensions.iter().find(|e| e.extension == ".rs");
        assert!(rs_stat.is_some());
        assert_eq!(rs_stat.unwrap().total_bytes, 200);
        assert_eq!(rs_stat.unwrap().file_count, 1);
    }
}
