use std::cmp::Reverse;
use std::collections::BinaryHeap;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use crate::arena::{ARENA_FLAG_ROOT, DirArena, LocalTopFiles, TopFileCandidate};
use crate::work_stealing::{Steal, Worker};

use super::buffer::DualBuffer;
use super::state::{GlobalState, ThreadLocalResult};
use super::{ProgressCallback, ScanOptions, ScanResult, normalize_scan_path};

pub fn worker_loop(
    thread_id: usize,
    worker: Worker<PathBuf>,
    state: Arc<GlobalState>,
) -> ThreadLocalResult {
    crate::sys::pin_thread_to_core(thread_id);

    let mut local_arena = DirArena::new();
    let mut local_top_files = LocalTopFiles::new(state.config.top_limit);
    let mut dual_buffer = DualBuffer::new(128 * 1024, 4096);
    let mut path_stack = Vec::with_capacity(1024);
    #[cfg(windows)]
    let mut wide_path_stack = super::windows::WidePathStack::new();
    #[cfg(windows)]
    let mut utf8_scratch = Vec::with_capacity(512);
    let mut local_files: u64 = 0;
    let mut local_bytes: u64 = 0;
    let mut local_ext_stats = std::collections::HashMap::new();

    let num_stealers = state.stealers.len();
    let mut is_active = true;

    loop {
        if state.cancel_requested.load(Ordering::Relaxed) {
            break;
        }

        while state.pause_requested.load(Ordering::Relaxed) {
            let guard = state.cvar_mutex.lock().unwrap();
            if is_active {
                state.active_workers.fetch_sub(1, Ordering::SeqCst);
                is_active = false;
            }
            let _ = state.cvar.wait_timeout(guard, Duration::from_millis(50));
            if state.cancel_requested.load(Ordering::Relaxed) {
                break;
            }
        }

        let task = if let Some(t) = worker.pop() {
            Some(t)
        } else {
            let mut stolen = None;
            for offset in 1..num_stealers {
                let target_idx = (thread_id + offset) % num_stealers;
                match state.stealers[target_idx].steal() {
                    Steal::Success(t) => {
                        stolen = Some(t);
                        break;
                    }
                    Steal::Retry | Steal::Empty => continue,
                }
            }
            stolen
        };

        if let Some(current_dir) = task {
            if !is_active {
                state.active_workers.fetch_add(1, Ordering::SeqCst);
                is_active = true;
            }

            if let Ok(mut act) = state.current_active.try_lock() {
                *act = current_dir.to_string_lossy().to_string();
            }

            let cur_depth = current_dir.components().count();
            let rel_depth = cur_depth.saturating_sub(state.config.base_depth);

            #[cfg(unix)]
            let root_name = current_dir.as_os_str().as_bytes();
            #[cfg(windows)]
            let root_name_str = current_dir.to_string_lossy();
            #[cfg(windows)]
            let root_name = root_name_str.as_bytes();

            let task_node = local_arena.add_root(rel_depth as u16, root_name);

            #[cfg(unix)]
            super::linux::scan_directory_tree(
                &current_dir,
                None,
                task_node,
                rel_depth,
                &state,
                &worker,
                &mut dual_buffer,
                &mut path_stack,
                &mut local_arena,
                &mut local_top_files,
                &mut local_ext_stats,
                &mut local_files,
                &mut local_bytes,
            );

            #[cfg(windows)]
            {
                wide_path_stack.set_root(&current_dir);
                path_stack.clear();
                path_stack.extend_from_slice(root_name);
                while path_stack.len() > 3
                    && (path_stack.ends_with(b"\\") || path_stack.ends_with(b"/"))
                {
                    path_stack.pop();
                }

                super::windows::scan_directory_tree_windows(
                    task_node,
                    rel_depth,
                    &state,
                    &worker,
                    &mut dual_buffer,
                    &mut wide_path_stack,
                    &mut path_stack,
                    &mut utf8_scratch,
                    &mut local_arena,
                    &mut local_top_files,
                    &mut local_ext_stats,
                    &mut local_files,
                    &mut local_bytes,
                );
            }
        } else {
            // Flush remaining counts when transitioning to idle
            if local_files > 0 {
                state.total_bytes.fetch_add(local_bytes, Ordering::Relaxed);
                state.total_files.fetch_add(local_files, Ordering::Relaxed);
                local_bytes = 0;
                local_files = 0;
            }

            let guard = state.cvar_mutex.lock().unwrap();
            if is_active {
                state.active_workers.fetch_sub(1, Ordering::SeqCst);
                is_active = false;
            }

            if state.done.load(Ordering::SeqCst) {
                break;
            }

            if state.active_workers.load(Ordering::SeqCst) == 0 && state.all_stealers_empty() {
                state.done.store(true, Ordering::SeqCst);
                state.cvar.notify_all();
                break;
            }

            let _ = state.cvar.wait_timeout(guard, Duration::from_millis(1));
            if state.done.load(Ordering::SeqCst) {
                break;
            } else if !is_active {
                state.active_workers.fetch_add(1, Ordering::SeqCst);
                is_active = true;
            }
        }
    }

    if local_files > 0 {
        state.total_bytes.fetch_add(local_bytes, Ordering::Relaxed);
        state.total_files.fetch_add(local_files, Ordering::Relaxed);
    }

    ThreadLocalResult {
        top_files: local_top_files,
        arena: local_arena,
        ext_stats: local_ext_stats,
    }
}

pub fn execute_workers_and_rollup(
    state: Arc<GlobalState>,
    workers: Vec<Worker<PathBuf>>,
    root: &Path,
    options: &ScanOptions,
    progress_cb: Option<ProgressCallback>,
) -> ScanResult {
    let num_threads = workers.len();
    let start_time = Instant::now();

    let rep_handle = if let Some(cb) = progress_cb {
        let rep_state = Arc::clone(&state);
        Some(thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(60));
                if rep_state.done.load(Ordering::SeqCst) {
                    break;
                }

                let cur_files = rep_state.total_files.load(Ordering::Relaxed);
                let cur_bytes = rep_state.total_bytes.load(Ordering::Relaxed);
                let active = rep_state.active_workers.load(Ordering::Relaxed);
                let active_path = rep_state
                    .current_active
                    .try_lock()
                    .map(|g| g.clone())
                    .unwrap_or_default();

                cb(cur_bytes, cur_files, active, &active_path);
            }
        }))
    } else {
        None
    };

    let mut handles = Vec::with_capacity(num_threads);
    for (i, worker) in workers.into_iter().enumerate() {
        let st = Arc::clone(&state);
        handles.push(thread::spawn(move || worker_loop(i, worker, st)));
    }

    let mut all_results = Vec::with_capacity(num_threads);
    for h in handles {
        if let Ok(res) = h.join() {
            all_results.push(res);
        }
    }

    state.done.store(true, Ordering::SeqCst);
    state.cvar.notify_all();
    if let Some(h) = rep_handle {
        let _ = h.join();
    }

    let elapsed = start_time.elapsed();
    let total_bytes = state.total_bytes.load(Ordering::SeqCst);
    let total_files = state.total_files.load(Ordering::SeqCst);

    // Merge extension statistics
    let mut merged_ext_stats: std::collections::HashMap<String, (u64, u64)> =
        std::collections::HashMap::new();
    for res in &all_results {
        for (k, v) in &res.ext_stats {
            let entry = merged_ext_stats.entry(k.clone()).or_insert((0, 0));
            entry.0 += v.0;
            entry.1 += v.1;
        }
    }

    // Zero-copy subtree merging into master arena
    let norm_root = normalize_scan_path(root);
    #[cfg(unix)]
    let norm_root_bytes = norm_root.as_os_str().as_bytes();
    #[cfg(windows)]
    let norm_root_str = norm_root.to_string_lossy();
    #[cfg(windows)]
    let norm_root_bytes = norm_root_str.as_bytes();

    let total_nodes_est: usize = all_results.iter().map(|r| r.arena.len()).sum();
    let total_names_est: usize = all_results.iter().map(|r| r.arena.names.len()).sum();
    let mut master_arena =
        DirArena::with_capacity(total_nodes_est.max(16), total_names_est.max(256));

    let mut master_names_base_offsets: Vec<u32> = Vec::with_capacity(all_results.len());
    for res in &all_results {
        master_names_base_offsets.push(master_arena.names.len() as u32);
        master_arena.names.extend_from_slice(&res.arena.names);
    }

    let mut all_remappings: Vec<Vec<u32>> = all_results
        .iter()
        .map(|r| vec![u32::MAX; r.arena.len()])
        .collect();

    struct SubtreeSlice {
        worker_idx: usize,
        start_idx: usize,
        end_idx: usize,
        task_path: PathBuf,
        depth: usize,
    }

    let mut slices = Vec::new();
    for (worker_idx, res) in all_results.iter().enumerate() {
        let nodes = &res.arena.nodes;
        if nodes.is_empty() {
            continue;
        }
        let mut root_indices = Vec::new();
        for (i, node) in nodes.iter().enumerate() {
            if (node.flags & ARENA_FLAG_ROOT) != 0 || node.parent_idx == i as u32 {
                root_indices.push(i);
            }
        }
        for (k, &start_idx) in root_indices.iter().enumerate() {
            let end_idx = if k + 1 < root_indices.len() {
                root_indices[k + 1]
            } else {
                nodes.len()
            };
            let path_bytes = res.arena.name_of(start_idx as u32);
            #[cfg(unix)]
            let p = PathBuf::from(std::ffi::OsStr::from_bytes(path_bytes));
            #[cfg(windows)]
            let p = PathBuf::from(String::from_utf8_lossy(path_bytes).as_ref());
            let norm_p = normalize_scan_path(&p);
            let depth = norm_p.components().count();
            slices.push(SubtreeSlice {
                worker_idx,
                start_idx,
                end_idx,
                task_path: norm_p,
                depth,
            });
        }
    }

    // Sort slices by path depth ascending so parents are always merged before children
    slices.sort_by_key(|s| s.depth);

    let mut path_to_master_idx: std::collections::HashMap<PathBuf, u32> =
        std::collections::HashMap::with_capacity(total_nodes_est.max(16));

    let master_root_idx = master_arena.add_root(0, norm_root_bytes);
    path_to_master_idx.insert(norm_root.clone(), master_root_idx);

    let mut path_scratch = Vec::with_capacity(512);

    for slice in slices {
        let worker_idx = slice.worker_idx;
        let name_base = master_names_base_offsets[worker_idx];

        if slice.task_path == norm_root {
            // Merging scan root itself
            master_arena.merge_subtree(
                &all_results[worker_idx].arena,
                slice.start_idx,
                slice.end_idx,
                master_root_idx,
                None,
                false,
                name_base,
                &mut all_remappings[worker_idx],
            );
        } else {
            // Merging a stolen subdirectory task
            let parent_path = match slice.task_path.parent() {
                Some(p) if !p.as_os_str().is_empty() => p,
                _ => Path::new("."),
            };
            let norm_parent = normalize_scan_path(parent_path);

            // Ensure parent and ancestor nodes exist in master_arena
            let parent_master_idx = if let Some(&p_idx) = path_to_master_idx.get(&norm_parent) {
                p_idx
            } else {
                let mut current_ancestor = norm_root.clone();
                let mut current_parent_idx = master_root_idx;
                if let Ok(rel) = norm_parent.strip_prefix(&norm_root) {
                    for comp in rel.components() {
                        let name_os = comp.as_os_str();
                        #[cfg(unix)]
                        let comp_bytes = name_os.as_bytes();
                        #[cfg(windows)]
                        let comp_str = name_os.to_string_lossy();
                        #[cfg(windows)]
                        let comp_bytes = comp_str.as_bytes();

                        current_ancestor.push(name_os);
                        let comp_norm = normalize_scan_path(&current_ancestor);
                        if let Some(&existing_idx) = path_to_master_idx.get(&comp_norm) {
                            current_parent_idx = existing_idx;
                        } else {
                            let depth = comp_norm
                                .components()
                                .count()
                                .saturating_sub(norm_root.components().count());
                            let new_node_idx =
                                master_arena.add_node(current_parent_idx, depth as u16, comp_bytes);
                            path_to_master_idx.insert(comp_norm, new_node_idx);
                            current_parent_idx = new_node_idx;
                        }
                    }
                }
                current_parent_idx
            };

            let file_name = slice.task_path.file_name().map(|f| {
                #[cfg(unix)]
                {
                    f.as_bytes()
                }
                #[cfg(windows)]
                {
                    f.to_string_lossy().into_owned().into_bytes()
                }
            });

            #[cfg(unix)]
            let file_name_bytes = file_name;
            #[cfg(windows)]
            let file_name_bytes = file_name.as_deref();

            if let Some(&existing_idx) = path_to_master_idx.get(&slice.task_path) {
                master_arena.merge_subtree(
                    &all_results[worker_idx].arena,
                    slice.start_idx,
                    slice.end_idx,
                    existing_idx,
                    None,
                    false,
                    name_base,
                    &mut all_remappings[worker_idx],
                );
            } else {
                let new_master_idx = master_arena.merge_subtree(
                    &all_results[worker_idx].arena,
                    slice.start_idx,
                    slice.end_idx,
                    parent_master_idx,
                    file_name_bytes,
                    true,
                    name_base,
                    &mut all_remappings[worker_idx],
                );
                path_to_master_idx.insert(slice.task_path.clone(), new_master_idx);
            }
        }

        // Register all child paths added in this slice into path_to_master_idx
        for &m_idx in &all_remappings[worker_idx][(slice.start_idx + 1)..slice.end_idx] {
            if m_idx != u32::MAX {
                path_scratch.clear();
                master_arena.reconstruct_path(m_idx, &mut path_scratch);
                #[cfg(unix)]
                let cp = PathBuf::from(std::ffi::OsStr::from_bytes(&path_scratch));
                #[cfg(windows)]
                let cp = PathBuf::from(String::from_utf8_lossy(&path_scratch).as_ref());
                path_to_master_idx.insert(normalize_scan_path(&cp), m_idx);
            }
        }
    }

    // O(N) reverse array rollup
    master_arena.rollup();

    // Merge and remap top files
    let mut merged_top_heap: BinaryHeap<Reverse<TopFileCandidate>> =
        BinaryHeap::with_capacity(options.top_limit + 16);

    for (worker_idx, res) in all_results.iter().enumerate() {
        let remapping = &all_remappings[worker_idx];
        let name_base = master_names_base_offsets[worker_idx];
        for Reverse(mut cand) in res.top_files.heap.clone() {
            if (cand.dir_node_idx as usize) < remapping.len() {
                let remapped = remapping[cand.dir_node_idx as usize];
                if remapped != u32::MAX {
                    cand.dir_node_idx = remapped;
                }
            }
            if cand.is_spill == 1 {
                cand.spill_offset += name_base;
            }
            if options.top_limit > 0 {
                if merged_top_heap.len() >= options.top_limit {
                    if let Some(Reverse(min_cand)) = merged_top_heap.peek()
                        && cand.size <= min_cand.size
                    {
                        continue;
                    }
                    merged_top_heap.pop();
                }
                merged_top_heap.push(Reverse(cand));
            }
        }
    }

    // Format all_dirs and extract top_dirs
    let mut all_dirs: Vec<(PathBuf, u64)> = Vec::with_capacity(master_arena.len());
    for (idx, node) in master_arena.nodes.iter().enumerate() {
        if (node.rel_depth as usize) <= options.max_depth {
            path_scratch.clear();
            master_arena.reconstruct_path(idx as u32, &mut path_scratch);
            #[cfg(unix)]
            let p = PathBuf::from(std::ffi::OsStr::from_bytes(&path_scratch));
            #[cfg(windows)]
            let p = PathBuf::from(String::from_utf8_lossy(&path_scratch).as_ref());
            let norm_p = normalize_scan_path(&p);
            all_dirs.push((norm_p, node.total_bytes));
        }
    }
    all_dirs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let max_dir_size = all_dirs.first().map(|(_, s)| *s).unwrap_or(1).max(1);
    let top_dirs: Vec<(PathBuf, u64)> = all_dirs.iter().take(options.top_limit).cloned().collect();

    // Resolve top files
    let mut files_vec: Vec<(u64, PathBuf)> = Vec::with_capacity(merged_top_heap.len());
    let mut file_path_buf = Vec::with_capacity(512);
    for Reverse(cand) in merged_top_heap {
        file_path_buf.clear();
        master_arena.resolve_top_file_path(&cand, &mut file_path_buf);
        #[cfg(unix)]
        let pb = PathBuf::from(std::ffi::OsStr::from_bytes(&file_path_buf));
        #[cfg(windows)]
        let pb = PathBuf::from(String::from_utf8_lossy(&file_path_buf).as_ref());
        let norm_pb = normalize_scan_path(&pb);
        files_vec.push((cand.size, norm_pb));
    }
    files_vec.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    files_vec.dedup_by(|a, b| a.1 == b.1);
    let max_file_size = files_vec.first().map(|(s, _)| *s).unwrap_or(1).max(1);
    let top_files: Vec<(u64, PathBuf)> = files_vec.into_iter().take(options.top_limit).collect();

    ScanResult {
        elapsed,
        total_bytes,
        total_files,
        top_dirs,
        top_files,
        max_dir_size,
        max_file_size,
        arenas: vec![master_arena],
        root: root.to_path_buf(),
        extension_stats: merged_ext_stats,
        all_dirs,
    }
}
