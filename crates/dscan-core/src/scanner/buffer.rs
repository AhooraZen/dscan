#[derive(Debug)]
pub enum BufferSource {
    #[cfg(target_os = "linux")]
    HugePage {
        ptr: *mut u8,
        size: usize,
    },
    #[cfg(target_os = "linux")]
    PrePopulatedMmap {
        ptr: *mut u8,
        size: usize,
    },
    #[cfg(windows)]
    VirtualAlloc {
        ptr: *mut u8,
        #[allow(dead_code)]
        size: usize,
        is_large_page: bool,
    },
    HeapLayout(std::alloc::Layout),
}

pub struct AlignedBuffer {
    ptr: *mut u8,
    source: BufferSource,
    len: usize,
}

impl AlignedBuffer {
    pub fn new(size: usize, align: usize) -> Self {
        #[cfg(target_os = "linux")]
        {
            // Try 2 MiB huge page allocation if size matches or exceeds 2 MiB
            if size >= 2 * 1024 * 1024 {
                // SAFETY: MAP_HUGETLB with anonymous private mapping.
                let ptr = unsafe {
                    crate::sys::mmap(
                        std::ptr::null_mut(),
                        size,
                        crate::sys::PROT_READ | crate::sys::PROT_WRITE,
                        crate::sys::MAP_PRIVATE
                            | crate::sys::MAP_ANONYMOUS
                            | crate::sys::MAP_HUGETLB
                            | crate::sys::MAP_POPULATE,
                        -1,
                        0,
                    )
                };
                if ptr != crate::sys::MAP_FAILED && !ptr.is_null() {
                    return AlignedBuffer {
                        ptr: ptr as *mut u8,
                        source: BufferSource::HugePage {
                            ptr: ptr as *mut u8,
                            size,
                        },
                        len: size,
                    };
                }
            }

            // Fallback to pre-populated anonymous mmap with transparent huge page madvise
            // SAFETY: Anonymous private mapping with MAP_POPULATE.
            let ptr = unsafe {
                crate::sys::mmap(
                    std::ptr::null_mut(),
                    size,
                    crate::sys::PROT_READ | crate::sys::PROT_WRITE,
                    crate::sys::MAP_PRIVATE | crate::sys::MAP_ANONYMOUS | crate::sys::MAP_POPULATE,
                    -1,
                    0,
                )
            };
            if ptr != crate::sys::MAP_FAILED && !ptr.is_null() {
                // SAFETY: ptr is valid mapped memory.
                unsafe {
                    crate::sys::madvise(ptr, size, crate::sys::MADV_HUGEPAGE);
                }
                return AlignedBuffer {
                    ptr: ptr as *mut u8,
                    source: BufferSource::PrePopulatedMmap {
                        ptr: ptr as *mut u8,
                        size,
                    },
                    len: size,
                };
            }
        }

        #[cfg(windows)]
        {
            use crate::sys::windows::*;
            // Attempt 2 MiB Large Page allocation if size is suitable
            let large_page_min = unsafe { GetLargePageMinimum() };
            if large_page_min > 0 && size >= large_page_min && size.is_multiple_of(large_page_min) {
                // SAFETY: VirtualAlloc with MEM_COMMIT | MEM_RESERVE | MEM_LARGE_PAGES.
                let ptr = unsafe {
                    VirtualAlloc(
                        std::ptr::null_mut(),
                        size,
                        MEM_COMMIT | MEM_RESERVE | MEM_LARGE_PAGES,
                        PAGE_READWRITE,
                    )
                };
                if !ptr.is_null() {
                    return AlignedBuffer {
                        ptr: ptr as *mut u8,
                        source: BufferSource::VirtualAlloc {
                            ptr: ptr as *mut u8,
                            size,
                            is_large_page: true,
                        },
                        len: size,
                    };
                }
            }

            // Standard VirtualAlloc (page-aligned, avoids CRT heap contention)
            // SAFETY: VirtualAlloc with MEM_COMMIT | MEM_RESERVE.
            let ptr = unsafe {
                VirtualAlloc(
                    std::ptr::null_mut(),
                    size,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_READWRITE,
                )
            };
            if !ptr.is_null() {
                return AlignedBuffer {
                    ptr: ptr as *mut u8,
                    source: BufferSource::VirtualAlloc {
                        ptr: ptr as *mut u8,
                        size,
                        is_large_page: false,
                    },
                    len: size,
                };
            }
        }

        // Standard heap allocation fallback
        let layout = std::alloc::Layout::from_size_align(size, align).expect("valid layout");
        // SAFETY: layout has non-zero size and valid power-of-two alignment.
        let ptr = unsafe { std::alloc::alloc(layout) };
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        AlignedBuffer {
            ptr,
            source: BufferSource::HeapLayout(layout),
            len: size,
        }
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: self.ptr points to an allocated buffer of self.len bytes.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[cfg(windows)]
    pub fn is_large_page(&self) -> bool {
        matches!(
            self.source,
            BufferSource::VirtualAlloc {
                is_large_page: true,
                ..
            }
        )
    }
}

impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        match self.source {
            #[cfg(target_os = "linux")]
            BufferSource::HugePage { ptr, size } | BufferSource::PrePopulatedMmap { ptr, size } => {
                // SAFETY: ptr was allocated by mmap and size matches.
                unsafe {
                    crate::sys::munmap(ptr as *mut std::ffi::c_void, size);
                }
            }
            #[cfg(windows)]
            BufferSource::VirtualAlloc { ptr, .. } => {
                // SAFETY: ptr was allocated by VirtualAlloc with MEM_COMMIT | MEM_RESERVE.
                unsafe {
                    crate::sys::VirtualFree(
                        ptr as *mut std::ffi::c_void,
                        0,
                        crate::sys::MEM_RELEASE,
                    );
                }
            }
            BufferSource::HeapLayout(layout) => {
                // SAFETY: self.ptr was allocated with self.layout by std::alloc::alloc.
                unsafe { std::alloc::dealloc(self.ptr, layout) };
            }
        }
    }
}

// SAFETY: AlignedBuffer owns its memory allocation and can be transferred across threads.
unsafe impl Send for AlignedBuffer {}

pub struct DualBuffer {
    pub buf_a: AlignedBuffer,
    pub buf_b: AlignedBuffer,
    pub active_is_a: bool,
}

impl DualBuffer {
    pub fn new(capacity: usize, align: usize) -> Self {
        Self {
            buf_a: AlignedBuffer::new(capacity, align),
            buf_b: AlignedBuffer::new(capacity, align),
            active_is_a: true,
        }
    }

    #[inline(always)]
    pub fn current_mut(&mut self) -> &mut AlignedBuffer {
        if self.active_is_a {
            &mut self.buf_a
        } else {
            &mut self.buf_b
        }
    }

    #[inline(always)]
    pub fn swap(&mut self) {
        self.active_is_a = !self.active_is_a;
    }
}

#[derive(Debug, Default)]
pub struct FlatSubdirBuf {
    pub names: Vec<u8>,
    pub offsets: Vec<(u32, u16)>,
}

impl FlatSubdirBuf {
    pub fn new() -> Self {
        Self {
            names: Vec::with_capacity(4096),
            offsets: Vec::with_capacity(128),
        }
    }

    pub fn with_capacity(names_cap: usize, offsets_cap: usize) -> Self {
        Self {
            names: Vec::with_capacity(names_cap),
            offsets: Vec::with_capacity(offsets_cap),
        }
    }

    #[inline(always)]
    pub fn push(&mut self, name: &[u8]) {
        let offset = self.names.len() as u32;
        let len = name.len() as u16;
        self.names.extend_from_slice(name);
        self.offsets.push((offset, len));
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.names.clear();
        self.offsets.clear();
    }

    #[inline(always)]
    pub fn truncate(&mut self, len: usize) {
        if len < self.offsets.len() {
            if len == 0 {
                self.names.clear();
                self.offsets.clear();
            } else {
                let (offset, _) = self.offsets[len];
                self.names.truncate(offset as usize);
                self.offsets.truncate(len);
            }
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    #[inline(always)]
    pub fn get(&self, idx: usize) -> Option<&[u8]> {
        let &(offset, len) = self.offsets.get(idx)?;
        let start = offset as usize;
        let end = start + len as usize;
        if end <= self.names.len() {
            Some(&self.names[start..end])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aligned_buffer_alloc_and_slice() {
        let mut buf = AlignedBuffer::new(65536, 4096);
        assert_eq!(buf.len(), 65536);
        assert!(!buf.is_empty());
        let slice = buf.as_mut_slice();
        slice[0] = 0xAA;
        slice[65535] = 0x55;
        assert_eq!(slice[0], 0xAA);
        assert_eq!(slice[65535], 0x55);
    }

    #[test]
    fn test_dual_buffer() {
        let mut dual = DualBuffer::new(4096, 64);
        assert!(dual.active_is_a);
        dual.current_mut().as_mut_slice()[0] = 42;
        dual.swap();
        assert!(!dual.active_is_a);
        dual.current_mut().as_mut_slice()[0] = 99;
        assert_eq!(dual.buf_a.as_mut_slice()[0], 42);
        assert_eq!(dual.buf_b.as_mut_slice()[0], 99);
    }

    #[test]
    fn test_flat_subdir_buf_append_and_clear() {
        let mut buf = FlatSubdirBuf::new();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);

        buf.push(b"node_modules");
        buf.push(b"target");
        buf.push(b"src");

        assert_eq!(buf.len(), 3);
        assert_eq!(buf.get(0), Some(&b"node_modules"[..]));
        assert_eq!(buf.get(1), Some(&b"target"[..]));
        assert_eq!(buf.get(2), Some(&b"src"[..]));
        assert_eq!(buf.get(3), None);

        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);

        buf.push(b"first");
        buf.push(b"second");
        buf.push(b"third");
        assert_eq!(buf.len(), 3);
        buf.truncate(2);
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.get(0), Some(&b"first"[..]));
        assert_eq!(buf.get(1), Some(&b"second"[..]));
        assert_eq!(buf.get(2), None);
        buf.truncate(0);
        assert_eq!(buf.len(), 0);
    }
}
