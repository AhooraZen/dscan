use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use crate::scanner::{
    GlobalState, ScanOptions, ScanResult, execute_workers_and_rollup, extract_file_extension,
    init_scan_state,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ScanProgressDto {
    pub total_bytes: u64,
    pub total_files: u64,
    pub active_workers: usize,
    pub current_path: String,
    pub elapsed_millis: u64,
    pub files_per_sec: f64,
    pub bytes_per_sec: f64,
    pub is_complete: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreemapNodeDto {
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

#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionStatDto {
    pub extension: String,
    pub total_bytes: u64,
    pub file_count: u64,
    pub percentage_of_total: f64,
}

pub struct ScanSession {
    state: Arc<GlobalState>,
    start_time: Instant,
    result: Arc<Mutex<Option<ScanResult>>>,
    coordinator_handle: Mutex<Option<thread::JoinHandle<()>>>,
}

impl ScanSession {
    pub fn start(options: ScanOptions) -> Result<Arc<Self>, std::io::Error> {
        let (state, workers) = init_scan_state(&options)?;
        let result = Arc::new(Mutex::new(None));
        let session = Arc::new(Self {
            state: Arc::clone(&state),
            start_time: Instant::now(),
            result: Arc::clone(&result),
            coordinator_handle: Mutex::new(None),
        });

        let res_clone = Arc::clone(&result);
        let state_clone = Arc::clone(&state);
        let opts = options.clone();
        let handle = thread::spawn(move || {
            let root = Path::new(&opts.target_path);
            let scan_res = execute_workers_and_rollup(state_clone, workers, root, &opts, None);
            *res_clone.lock().unwrap() = Some(scan_res);
        });

        *session.coordinator_handle.lock().unwrap() = Some(handle);
        Ok(session)
    }

    pub fn poll_progress(&self) -> ScanProgressDto {
        let elapsed = self.start_time.elapsed();
        let elapsed_millis = elapsed.as_millis() as u64;
        let elapsed_sec = elapsed.as_secs_f64();
        let total_bytes = self.state.total_bytes.load(Ordering::Relaxed);
        let total_files = self.state.total_files.load(Ordering::Relaxed);
        let active_workers = self.state.active_workers.load(Ordering::Relaxed);
        let is_complete = self.state.done.load(Ordering::Relaxed);
        let current_path = self
            .state
            .current_active
            .try_lock()
            .map(|g| g.clone())
            .unwrap_or_default();

        let files_per_sec = if elapsed_sec > 0.001 {
            total_files as f64 / elapsed_sec
        } else {
            0.0
        };
        let bytes_per_sec = if elapsed_sec > 0.001 {
            total_bytes as f64 / elapsed_sec
        } else {
            0.0
        };

        ScanProgressDto {
            total_bytes,
            total_files,
            active_workers,
            current_path,
            elapsed_millis,
            files_per_sec,
            bytes_per_sec,
            is_complete,
        }
    }

    pub fn cancel(&self) {
        self.state.cancel_requested.store(true, Ordering::SeqCst);
        self.state.cvar.notify_all();
    }

    pub fn pause(&self) {
        self.state.pause_requested.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.state.pause_requested.store(false, Ordering::SeqCst);
        self.state.cvar.notify_all();
    }

    pub fn is_paused(&self) -> bool {
        self.state.pause_requested.load(Ordering::Relaxed)
    }

    pub fn is_complete(&self) -> bool {
        self.state.done.load(Ordering::Relaxed)
    }

    pub fn wait_for_completion(&self) {
        while !self.state.done.load(Ordering::Relaxed) {
            thread::sleep(std::time::Duration::from_millis(10));
        }
        let handle = self.coordinator_handle.lock().unwrap().take();
        if let Some(h) = handle {
            let _ = h.join();
        }
    }

    pub fn get_hierarchical_view(&self, max_depth: u16, max_nodes: usize) -> Vec<TreemapNodeDto> {
        let guard = self.result.lock().unwrap();
        if let Some(ref res) = *guard {
            build_treemap_nodes(
                &res.root,
                &res.top_dirs,
                &res.top_files,
                res.total_bytes,
                max_depth,
                max_nodes,
            )
        } else {
            let root_str = &self.state.config.matcher;
            let _ = root_str;
            vec![TreemapNodeDto {
                id: 0,
                parent_id: 0,
                name: "root".to_string(),
                total_bytes: self.state.total_bytes.load(Ordering::Relaxed),
                direct_bytes: 0,
                rel_depth: 0,
                is_dir: true,
                extension: String::new(),
                children_ids: Vec::new(),
            }]
        }
    }

    pub fn get_extension_breakdown(&self, limit: usize) -> Vec<ExtensionStatDto> {
        let guard = self.result.lock().unwrap();
        if let Some(ref res) = *guard {
            build_extension_breakdown(&res.extension_stats, res.total_bytes, limit)
        } else {
            Vec::new()
        }
    }
}

pub fn build_extension_breakdown(
    ext_stats: &HashMap<String, (u64, u64)>,
    grand_total: u64,
    limit: usize,
) -> Vec<ExtensionStatDto> {
    let mut list: Vec<ExtensionStatDto> = ext_stats
        .iter()
        .map(|(ext, &(bytes, count))| {
            let pct = if grand_total > 0 {
                (bytes as f64 / grand_total as f64) * 100.0
            } else {
                0.0
            };
            ExtensionStatDto {
                extension: ext.clone(),
                total_bytes: bytes,
                file_count: count,
                percentage_of_total: pct,
            }
        })
        .collect();

    list.sort_by(|a, b| {
        b.total_bytes
            .cmp(&a.total_bytes)
            .then_with(|| a.extension.cmp(&b.extension))
    });

    if limit > 0 && list.len() > limit {
        let (kept, rest) = list.split_at(limit);
        let mut result = kept.to_vec();
        let other_bytes: u64 = rest.iter().map(|s| s.total_bytes).sum();
        let other_count: u64 = rest.iter().map(|s| s.file_count).sum();
        let other_pct: f64 = rest.iter().map(|s| s.percentage_of_total).sum();
        result.push(ExtensionStatDto {
            extension: "[other]".to_string(),
            total_bytes: other_bytes,
            file_count: other_count,
            percentage_of_total: other_pct,
        });
        result
    } else {
        list
    }
}

pub fn build_treemap_nodes(
    root_path: &Path,
    top_dirs: &[(PathBuf, u64)],
    top_files: &[(u64, PathBuf)],
    total_bytes: u64,
    max_depth: u16,
    max_nodes: usize,
) -> Vec<TreemapNodeDto> {
    let root_name = root_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| root_path.to_string_lossy().to_string());

    let mut nodes: Vec<TreemapNodeDto> = Vec::with_capacity(max_nodes.min(4096));
    nodes.push(TreemapNodeDto {
        id: 0,
        parent_id: 0,
        name: if root_name.is_empty() {
            "/".to_string()
        } else {
            root_name
        },
        total_bytes,
        direct_bytes: 0,
        rel_depth: 0,
        is_dir: true,
        extension: String::new(),
        children_ids: Vec::new(),
    });

    let mut path_to_id: HashMap<PathBuf, u32> = HashMap::new();
    path_to_id.insert(PathBuf::new(), 0);

    // 1. Insert hierarchical directory structure
    for (dir_path, dir_bytes) in top_dirs {
        let Ok(rel) = dir_path.strip_prefix(root_path) else {
            continue;
        };
        if rel.as_os_str().is_empty() {
            continue;
        }

        let mut current_ancestor = PathBuf::new();
        let mut parent_id = 0u32;
        let mut depth = 0u16;

        for component in rel.components() {
            depth += 1;
            if depth > max_depth {
                break;
            }
            let comp_str = component.as_os_str().to_string_lossy().to_string();
            current_ancestor.push(component);

            if let Some(&existing_id) = path_to_id.get(&current_ancestor) {
                parent_id = existing_id;
            } else {
                if nodes.len() >= max_nodes {
                    break;
                }
                let node_id = nodes.len() as u32;
                nodes.push(TreemapNodeDto {
                    id: node_id,
                    parent_id,
                    name: comp_str,
                    total_bytes: *dir_bytes,
                    direct_bytes: 0,
                    rel_depth: depth,
                    is_dir: true,
                    extension: String::new(),
                    children_ids: Vec::new(),
                });
                nodes[parent_id as usize].children_ids.push(node_id);
                path_to_id.insert(current_ancestor.clone(), node_id);
                parent_id = node_id;
            }
        }
    }

    // 2. Insert top files under their respective parent directories
    for (file_size, file_path) in top_files {
        if nodes.len() >= max_nodes {
            break;
        }
        let Ok(rel) = file_path.strip_prefix(root_path) else {
            continue;
        };
        let parent_rel = rel.parent().unwrap_or(Path::new(""));

        // Find closest existing directory ancestor
        let mut curr_parent: &Path = parent_rel;
        let parent_id = loop {
            if let Some(&id) = path_to_id.get(curr_parent) {
                break id;
            }
            match curr_parent.parent() {
                Some(p) if !p.as_os_str().is_empty() => curr_parent = p,
                _ => break 0,
            }
        };

        let parent_depth = nodes[parent_id as usize].rel_depth;
        if parent_depth >= max_depth {
            continue;
        }

        let file_name = rel
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let ext = extract_file_extension(file_name.as_bytes());

        let file_node_id = nodes.len() as u32;
        nodes.push(TreemapNodeDto {
            id: file_node_id,
            parent_id,
            name: file_name,
            total_bytes: *file_size,
            direct_bytes: *file_size,
            rel_depth: parent_depth + 1,
            is_dir: false,
            extension: ext,
            children_ids: Vec::new(),
        });
        nodes[parent_id as usize].children_ids.push(file_node_id);
    }

    // 3. Balance subtree sizes so parent total_bytes = sum(children total_bytes)
    for dir_id in (0..nodes.len()).rev() {
        if !nodes[dir_id].is_dir {
            continue;
        }

        let children_ids = nodes[dir_id].children_ids.clone();
        if children_ids.is_empty() {
            continue;
        }

        let children_sum: u64 = children_ids
            .iter()
            .map(|&cid| nodes[cid as usize].total_bytes)
            .sum();

        if nodes[dir_id].total_bytes > children_sum && nodes.len() < max_nodes {
            let diff = nodes[dir_id].total_bytes - children_sum;
            let other_id = nodes.len() as u32;
            let pdepth = nodes[dir_id].rel_depth;
            nodes.push(TreemapNodeDto {
                id: other_id,
                parent_id: dir_id as u32,
                name: "[other files]".to_string(),
                total_bytes: diff,
                direct_bytes: diff,
                rel_depth: pdepth + 1,
                is_dir: false,
                extension: "[misc]".to_string(),
                children_ids: Vec::new(),
            });
            nodes[dir_id].children_ids.push(other_id);
            nodes[dir_id].direct_bytes = diff;
        } else if children_sum > nodes[dir_id].total_bytes {
            nodes[dir_id].total_bytes = children_sum;
        }
    }

    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_snapshot_dto_creation() {
        let progress = ScanProgressDto {
            total_bytes: 1048576,
            total_files: 512,
            active_workers: 4,
            current_path: "/home/test".to_string(),
            elapsed_millis: 1500,
            files_per_sec: 341.3,
            bytes_per_sec: 699050.6,
            is_complete: true,
        };
        assert_eq!(progress.total_bytes, 1048576);
        assert_eq!(progress.total_files, 512);
        assert!(progress.is_complete);
    }

    #[test]
    fn test_hierarchical_view_generation() {
        let root = PathBuf::from("/test/root");
        let top_dirs = vec![
            (PathBuf::from("/test/root/docs"), 500_000),
            (PathBuf::from("/test/root/media"), 1_000_000),
        ];
        let top_files = vec![
            (800_000, PathBuf::from("/test/root/media/video.mp4")),
            (200_000, PathBuf::from("/test/root/docs/paper.pdf")),
        ];

        let nodes = build_treemap_nodes(&root, &top_dirs, &top_files, 1_500_000, 4, 100);

        assert!(!nodes.is_empty());
        assert_eq!(nodes[0].id, 0);
        assert_eq!(nodes[0].name, "root");
        assert!(nodes[0].is_dir);

        // Find video.mp4
        let video = nodes.iter().find(|n| n.name == "video.mp4");
        assert!(video.is_some());
        let v = video.unwrap();
        assert_eq!(v.extension, ".mp4");
        assert_eq!(v.total_bytes, 800_000);
        assert!(!v.is_dir);

        // Check parent-child invariant for root
        let root_children_sum: u64 = nodes[0]
            .children_ids
            .iter()
            .map(|&id| nodes[id as usize].total_bytes)
            .sum();
        assert_eq!(root_children_sum, nodes[0].total_bytes);
    }

    #[test]
    fn test_extension_breakdown_aggregation() {
        let mut stats = HashMap::new();
        stats.insert(".mp4".to_string(), (1000, 2));
        stats.insert(".rs".to_string(), (500, 10));
        stats.insert(".txt".to_string(), (200, 5));

        let res = build_extension_breakdown(&stats, 1700, 2);
        assert_eq!(res.len(), 3); // 2 kept + 1 [other]
        assert_eq!(res[0].extension, ".mp4");
        assert_eq!(res[0].total_bytes, 1000);
        assert_eq!(res[1].extension, ".rs");
        assert_eq!(res[2].extension, "[other]");
        assert_eq!(res[2].total_bytes, 200);
    }

    #[test]
    fn test_scan_session_synthetic_tree() {
        let temp_dir =
            std::env::temp_dir().join(format!("dscan_session_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("subdir")).unwrap();
        fs::write(temp_dir.join("file1.txt"), vec![b'A'; 4096]).unwrap();
        fs::write(temp_dir.join("subdir/file2.bin"), vec![b'B'; 8192]).unwrap();

        let opts = ScanOptions {
            target_path: temp_dir.to_str().unwrap().to_string(),
            top_limit: 10,
            max_depth: 3,
            threads: 2,
            excludes: vec![],
            follow_symlinks: false,
            cross_filesystems: false,
            collect_ext_stats: true,
        };

        let session = ScanSession::start(opts).expect("ScanSession should start");
        session.wait_for_completion();

        assert!(session.is_complete());
        let progress = session.poll_progress();
        assert!(progress.is_complete);
        assert!(progress.total_bytes >= 12288);
        assert!(progress.total_files >= 2);

        let treemap = session.get_hierarchical_view(3, 50);
        assert!(!treemap.is_empty());
        assert_eq!(treemap[0].id, 0);

        let exts = session.get_extension_breakdown(10);
        assert!(!exts.is_empty());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
