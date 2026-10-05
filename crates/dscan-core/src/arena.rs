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

        for (i, &idx) in chain.iter().enumerate() {
            let name = self.name_of(idx);
            if i > 0 && !out.ends_with(b"/") && !name.starts_with(b"/") {
                out.push(b'/');
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
}
