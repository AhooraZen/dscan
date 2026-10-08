use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub const ARENA_FLAG_ROOT: u16 = 1 << 0;

pub const TOP_FILE_INLINE_MAX: usize = 48;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TopFileCandidate {
    pub size: u64,
    pub dir_node_idx: u32,
    pub name_len: u16,
    pub is_spill: u8,
    pub name_inline: [u8; TOP_FILE_INLINE_MAX],
    pub spill_offset: u32,
}

impl Ord for TopFileCandidate {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.size
            .cmp(&other.size)
            .then_with(|| self.dir_node_idx.cmp(&other.dir_node_idx))
            .then_with(|| self.name_len.cmp(&other.name_len))
            .then_with(|| self.name_inline.cmp(&other.name_inline))
            .then_with(|| self.spill_offset.cmp(&other.spill_offset))
    }
}

impl PartialOrd for TopFileCandidate {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone)]
pub struct LocalTopFiles {
    pub heap: BinaryHeap<Reverse<TopFileCandidate>>,
    pub limit: usize,
}

impl Default for LocalTopFiles {
    fn default() -> Self {
        Self::new(0)
    }
}

impl LocalTopFiles {
    pub fn new(limit: usize) -> Self {
        Self {
            heap: BinaryHeap::with_capacity(limit + 8),
            limit,
        }
    }

    #[inline(always)]
    pub fn push(&mut self, size: u64, dir_node_idx: u32, name: &[u8], arena: &mut DirArena) {
        if self.limit == 0 {
            return;
        }

        if self.heap.len() >= self.limit {
            if let Some(Reverse(min_cand)) = self.heap.peek()
                && size <= min_cand.size
            {
                return;
            }
            self.heap.pop();
        }

        let mut cand = TopFileCandidate {
            size,
            dir_node_idx,
            name_len: name.len() as u16,
            is_spill: 0,
            name_inline: [0u8; TOP_FILE_INLINE_MAX],
            spill_offset: 0,
        };

        if name.len() <= TOP_FILE_INLINE_MAX {
            cand.name_inline[..name.len()].copy_from_slice(name);
        } else {
            cand.is_spill = 1;
            cand.spill_offset = arena.names.len() as u32;
            arena.names.extend_from_slice(name);
        }

        self.heap.push(Reverse(cand));
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArenaNode {
    pub parent_idx: u32,
    pub rel_depth: u16,
    pub flags: u16,
    pub direct_bytes: u64,
    pub total_bytes: u64,
    pub name_offset: u32,
    pub name_len: u32,
}

#[derive(Debug, Clone)]
pub struct DirArena {
    pub nodes: Vec<ArenaNode>,
    pub names: Vec<u8>,
}

impl Default for DirArena {
    fn default() -> Self {
        Self::new()
    }
}

impl DirArena {
    pub fn new() -> Self {
        Self::with_capacity(4096, 65536)
    }

    pub fn with_capacity(nodes_cap: usize, names_cap: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(nodes_cap),
            names: Vec::with_capacity(names_cap),
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.names.clear();
    }

    #[inline]
    pub fn add_node(&mut self, parent_idx: u32, rel_depth: u16, name: &[u8]) -> u32 {
        let idx = self.nodes.len() as u32;
        let name_offset = self.names.len() as u32;
        let name_len = name.len() as u32;
        self.names.extend_from_slice(name);

        let flags = if parent_idx == idx {
            ARENA_FLAG_ROOT
        } else {
            0
        };

        self.nodes.push(ArenaNode {
            parent_idx,
            rel_depth,
            flags,
            direct_bytes: 0,
            total_bytes: 0,
            name_offset,
            name_len,
        });

        idx
    }

    #[inline]
    pub fn add_root(&mut self, rel_depth: u16, name: &[u8]) -> u32 {
        let idx = self.nodes.len() as u32;
        let name_offset = self.names.len() as u32;
        let name_len = name.len() as u32;
        self.names.extend_from_slice(name);

        self.nodes.push(ArenaNode {
            parent_idx: idx,
            rel_depth,
            flags: ARENA_FLAG_ROOT,
            direct_bytes: 0,
            total_bytes: 0,
            name_offset,
            name_len,
        });

        idx
    }

    #[inline(always)]
    pub fn add_direct_bytes(&mut self, node_idx: u32, bytes: u64) {
        if let Some(node) = self.nodes.get_mut(node_idx as usize) {
            node.direct_bytes += bytes;
        }
    }

    #[inline(always)]
    pub fn name_of(&self, node_idx: u32) -> &[u8] {
        if let Some(node) = self.nodes.get(node_idx as usize) {
            let start = node.name_offset as usize;
            let end = start + node.name_len as usize;
            if end <= self.names.len() {
                return &self.names[start..end];
            }
        }
        b""
    }

    /// O(N) cache-linear reverse rollup propagating leaf directory sizes to ancestors.
    pub fn rollup(&mut self) {
        for node in &mut self.nodes {
            node.total_bytes = node.direct_bytes;
        }

        for i in (0..self.nodes.len()).rev() {
            let node = &self.nodes[i];
            if (node.flags & ARENA_FLAG_ROOT) != 0 || node.parent_idx as usize == i {
                continue;
            }
            let parent = node.parent_idx as usize;
            if parent < self.nodes.len() {
                let child_total = self.nodes[i].total_bytes;
                self.nodes[parent].total_bytes += child_total;
            }
        }
    }

    /// Merges a slice of nodes `start_idx..end_idx` from `other` into `self`.
    ///
    /// If `attach_as_child` is `true`:
    /// - The subtree root node is added as a new child of `target_master_idx` in `self`.
    /// - If `name_override` is `Some(name)`, `name` is used as the node's name in `self`
    ///   (useful when worker root stored a full path instead of a relative component).
    ///
    /// If `attach_as_child` is `false`:
    /// - The subtree root node's `direct_bytes` are accumulated into `self.nodes[target_master_idx]`.
    /// - `node_remapping[start_idx] = target_master_idx`.
    /// - Only children in `(start_idx + 1)..end_idx` are added to `self`.
    ///
    /// `name_base` is the offset in `self.names` where `other.names` was copied.
    /// `node_remapping` is updated with `node_remapping[other_idx] = new_master_idx`.
    // Subtree grafting requires source slice bounds, target parent index, name override, attachment mode, and remapping buffers.
    #[allow(clippy::too_many_arguments)]
    pub fn merge_subtree(
        &mut self,
        other: &DirArena,
        start_idx: usize,
        end_idx: usize,
        target_master_idx: u32,
        name_override: Option<&[u8]>,
        attach_as_child: bool,
        name_base: u32,
        node_remapping: &mut [u32],
    ) -> u32 {
        if start_idx >= end_idx || start_idx >= other.nodes.len() {
            return target_master_idx;
        }
        let end_idx = end_idx.min(other.nodes.len());

        let root_node = other.nodes[start_idx];
        let master_root_idx = if attach_as_child {
            let (name_offset, name_len) = if let Some(override_name) = name_override {
                let off = self.names.len() as u32;
                self.names.extend_from_slice(override_name);
                (off, override_name.len() as u32)
            } else {
                (root_node.name_offset + name_base, root_node.name_len)
            };

            let new_idx = self.nodes.len() as u32;
            if start_idx < node_remapping.len() {
                node_remapping[start_idx] = new_idx;
            }
            self.nodes.push(ArenaNode {
                parent_idx: target_master_idx,
                rel_depth: root_node.rel_depth,
                flags: 0,
                direct_bytes: root_node.direct_bytes,
                total_bytes: 0,
                name_offset,
                name_len,
            });
            new_idx
        } else {
            if let Some(target) = self.nodes.get_mut(target_master_idx as usize) {
                target.direct_bytes += root_node.direct_bytes;
            }
            if start_idx < node_remapping.len() {
                node_remapping[start_idx] = target_master_idx;
            }
            target_master_idx
        };

        for i in (start_idx + 1)..end_idx {
            let node = other.nodes[i];
            let parent_master_idx = if (node.parent_idx as usize) < node_remapping.len() {
                let p = node_remapping[node.parent_idx as usize];
                if p == u32::MAX { master_root_idx } else { p }
            } else {
                master_root_idx
            };

            let new_idx = self.nodes.len() as u32;
            if i < node_remapping.len() {
                node_remapping[i] = new_idx;
            }
            self.nodes.push(ArenaNode {
                parent_idx: parent_master_idx,
                rel_depth: node.rel_depth,
                flags: 0,
                direct_bytes: node.direct_bytes,
                total_bytes: 0,
                name_offset: node.name_offset + name_base,
                name_len: node.name_len,
            });
        }

        master_root_idx
    }

    /// Reconstructs the full path for a node by climbing the parent index chain.
    pub fn reconstruct_path(&self, mut curr: u32, out: &mut Vec<u8>) {
        out.clear();
        let mut chain = Vec::with_capacity(16);
        while (curr as usize) < self.nodes.len() {
            chain.push(curr);
            let node = &self.nodes[curr as usize];
            if (node.flags & ARENA_FLAG_ROOT) != 0 || node.parent_idx == curr {
                break;
            }
            curr = node.parent_idx;
        }
        chain.reverse();

        let use_backslash = cfg!(windows)
            && (!chain.is_empty() && self.name_of(chain[0]).contains(&b'\\')
                || out.contains(&b'\\'));
        let sep: u8 = if use_backslash { b'\\' } else { b'/' };

        for (i, &idx) in chain.iter().enumerate() {
            let name = self.name_of(idx);
            if i > 0
                && !out.ends_with(b"/")
                && !out.ends_with(b"\\")
                && !name.starts_with(b"/")
                && !name.starts_with(b"\\")
            {
                out.push(sep);
            }
            out.extend_from_slice(name);
        }
    }

    /// Reconstructs the full absolute path for a top file candidate into `out`.
    pub fn resolve_top_file_path(&self, cand: &TopFileCandidate, out: &mut Vec<u8>) {
        self.reconstruct_path(cand.dir_node_idx, out);
        let sep = if cfg!(windows) && out.contains(&b'\\') {
            b'\\'
        } else {
            b'/'
        };
        if !out.ends_with(b"/") && !out.ends_with(b"\\") && !out.is_empty() {
            out.push(sep);
        }
        if cand.is_spill == 0 {
            let len = cand.name_len as usize;
            out.extend_from_slice(&cand.name_inline[..len]);
        } else {
            let start = cand.spill_offset as usize;
            let end = start + cand.name_len as usize;
            if end <= self.names.len() {
                out.extend_from_slice(&self.names[start..end]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_node_size() {
        assert_eq!(std::mem::size_of::<ArenaNode>(), 32);
    }

    #[test]
    fn test_arena_hierarchy_and_rollup() {
        let mut arena = DirArena::new();
        let root = arena.add_root(0, b"/usr");
        arena.add_direct_bytes(root, 100);

        let bin = arena.add_node(root, 1, b"bin");
        arena.add_direct_bytes(bin, 200);

        let lib = arena.add_node(root, 1, b"lib");
        arena.add_direct_bytes(lib, 300);

        let modules = arena.add_node(lib, 2, b"modules");
        arena.add_direct_bytes(modules, 400);

        arena.rollup();

        assert_eq!(arena.nodes[modules as usize].total_bytes, 400);
        assert_eq!(arena.nodes[lib as usize].total_bytes, 700);
        assert_eq!(arena.nodes[bin as usize].total_bytes, 200);
        assert_eq!(arena.nodes[root as usize].total_bytes, 1000);

        let mut path = Vec::new();
        arena.reconstruct_path(modules, &mut path);
        assert_eq!(path, b"/usr/lib/modules");

        arena.reconstruct_path(root, &mut path);
        assert_eq!(path, b"/usr");
    }

    #[test]
    fn test_arena_merge_subtree_and_rollup() {
        // Ground truth single-threaded arena
        let mut expected_arena = DirArena::new();
        let exp_root = expected_arena.add_root(0, b"/test");
        expected_arena.add_direct_bytes(exp_root, 10);
        let exp_a = expected_arena.add_node(exp_root, 1, b"a");
        expected_arena.add_direct_bytes(exp_a, 20);
        let exp_a1 = expected_arena.add_node(exp_a, 2, b"a1");
        expected_arena.add_direct_bytes(exp_a1, 30);
        let exp_b = expected_arena.add_node(exp_root, 1, b"b");
        expected_arena.add_direct_bytes(exp_b, 40);
        let exp_b1 = expected_arena.add_node(exp_b, 2, b"b1");
        expected_arena.add_direct_bytes(exp_b1, 50);

        expected_arena.rollup();

        assert_eq!(expected_arena.nodes[exp_root as usize].total_bytes, 150);
        assert_eq!(expected_arena.nodes[exp_a as usize].total_bytes, 50);
        assert_eq!(expected_arena.nodes[exp_a1 as usize].total_bytes, 30);
        assert_eq!(expected_arena.nodes[exp_b as usize].total_bytes, 90);
        assert_eq!(expected_arena.nodes[exp_b1 as usize].total_bytes, 50);

        // Multi-worker scenario:
        // Worker 0 scanned /test and /test/a/a1
        let mut worker_0 = DirArena::new();
        let w0_root = worker_0.add_root(0, b"/test");
        worker_0.add_direct_bytes(w0_root, 10);
        let w0_a = worker_0.add_node(w0_root, 1, b"a");
        worker_0.add_direct_bytes(w0_a, 20);
        let w0_a1 = worker_0.add_node(w0_a, 2, b"a1");
        worker_0.add_direct_bytes(w0_a1, 30);

        // Worker 1 scanned stolen task /test/b and /test/b/b1
        let mut worker_1 = DirArena::new();
        let w1_root = worker_1.add_root(1, b"/test/b");
        worker_1.add_direct_bytes(w1_root, 40);
        let w1_b1 = worker_1.add_node(w1_root, 2, b"b1");
        worker_1.add_direct_bytes(w1_b1, 50);

        // Master arena merges both
        let mut master_arena = DirArena::new();
        let m_root = master_arena.add_root(0, b"/test");

        let name_base_0 = master_arena.names.len() as u32;
        master_arena.names.extend_from_slice(&worker_0.names);
        let mut remapping_0 = vec![u32::MAX; worker_0.len()];
        master_arena.merge_subtree(
            &worker_0,
            0,
            worker_0.len(),
            m_root,
            None,
            false,
            name_base_0,
            &mut remapping_0,
        );

        let name_base_1 = master_arena.names.len() as u32;
        master_arena.names.extend_from_slice(&worker_1.names);
        let mut remapping_1 = vec![u32::MAX; worker_1.len()];
        master_arena.merge_subtree(
            &worker_1,
            0,
            worker_1.len(),
            m_root,
            Some(b"b"),
            true,
            name_base_1,
            &mut remapping_1,
        );

        master_arena.rollup();

        assert_eq!(master_arena.len(), 5);
        assert_eq!(master_arena.nodes[0].total_bytes, 150); // /test
        assert_eq!(master_arena.nodes[1].total_bytes, 50); // a
        assert_eq!(master_arena.nodes[2].total_bytes, 30); // a1
        assert_eq!(master_arena.nodes[3].total_bytes, 90); // b
        assert_eq!(master_arena.nodes[4].total_bytes, 50); // b1

        let mut path = Vec::new();
        master_arena.reconstruct_path(0, &mut path);
        assert_eq!(path, b"/test");
        master_arena.reconstruct_path(1, &mut path);
        assert_eq!(path, b"/test/a");
        master_arena.reconstruct_path(2, &mut path);
        assert_eq!(path, b"/test/a/a1");
        master_arena.reconstruct_path(3, &mut path);
        assert_eq!(path, b"/test/b");
        master_arena.reconstruct_path(4, &mut path);
        assert_eq!(path, b"/test/b/b1");
    }
}
