#[inline(always)]
fn find_nul_scalar(slice: &[u8]) -> usize {
    slice.iter().position(|&b| b == 0).unwrap_or(slice.len())
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn find_nul_avx2(slice: &[u8]) -> usize {
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::*;
    let len = slice.len();
    let mut offset = 0;
    // SAFETY: AVX2 intrinsics enabled via target_feature and validated at callsite.
    unsafe {
        let zero = _mm256_setzero_si256();

        while offset + 32 <= len {
            let chunk = _mm256_loadu_si256(slice.as_ptr().add(offset) as *const __m256i);
            let cmp = _mm256_cmpeq_epi8(chunk, zero);
            let mask = _mm256_movemask_epi8(cmp) as u32;
            if mask != 0 {
                return offset + mask.trailing_zeros() as usize;
            }
            offset += 32;
        }
    }

    while offset < len {
        if slice[offset] == 0 {
            return offset;
        }
        offset += 1;
    }
    len
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn find_nul_sse2(slice: &[u8]) -> usize {
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::*;
    let len = slice.len();
    let mut offset = 0;
    // SAFETY: SSE2 intrinsics enabled via target_feature and validated at callsite.
    unsafe {
        let zero = _mm_setzero_si128();

        while offset + 16 <= len {
            let chunk = _mm_loadu_si128(slice.as_ptr().add(offset) as *const __m128i);
            let cmp = _mm_cmpeq_epi8(chunk, zero);
            let mask = _mm_movemask_epi8(cmp) as u32;
            if mask != 0 {
                return offset + mask.trailing_zeros() as usize;
            }
            offset += 16;
        }
    }

    while offset < len {
        if slice[offset] == 0 {
            return offset;
        }
        offset += 1;
    }
    len
}

#[cfg(target_arch = "aarch64")]
unsafe fn find_nul_neon(slice: &[u8]) -> usize {
    #[cfg(target_arch = "aarch64")]
    use core::arch::aarch64::*;
    let len = slice.len();
    let mut offset = 0;
    // SAFETY: NEON intrinsics on aarch64 with bounded pointer access.
    unsafe {
        let zero = vdupq_n_u8(0);

        while offset + 16 <= len {
            let chunk = vld1q_u8(slice.as_ptr().add(offset));
            let cmp = vceqq_u8(chunk, zero);
            if vmaxvq_u8(cmp) != 0 {
                for i in 0..16 {
                    if slice[offset + i] == 0 {
                        return offset + i;
                    }
                }
            }
            offset += 16;
        }
    }

    while offset < len {
        if slice[offset] == 0 {
            return offset;
        }
        offset += 1;
    }
    len
}

/// Finds the index of the first null byte (0) in the slice.
/// Returns slice.len() if no null byte is found.
#[inline]
pub fn find_nul(slice: &[u8]) -> usize {
    if slice.is_empty() {
        return 0;
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: Checked runtime support for AVX2.
            return unsafe { find_nul_avx2(slice) };
        }
        if is_x86_feature_detected!("sse2") {
            // SAFETY: Checked runtime support for SSE2.
            return unsafe { find_nul_sse2(slice) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is baseline on aarch64.
        return unsafe { find_nul_neon(slice) };
    }

    #[allow(unreachable_code)]
    find_nul_scalar(slice)
}

const DOT_GIT_U32: u32 = u32::from_ne_bytes(*b".git");

#[derive(Clone, Debug)]
pub struct FastExclusionMatcher {
    relative: Vec<Vec<u8>>,
    absolute: Vec<Vec<u8>>,
    min_rel_len: usize,
    min_abs_len: usize,
    has_dot_git: bool,
}

impl FastExclusionMatcher {
    pub fn new(excludes: &[Vec<u8>]) -> Self {
        let mut relative = Vec::new();
        let mut absolute = Vec::new();
        let mut has_dot_git = false;

        for ex in excludes {
            if ex.is_empty() {
                continue;
            }
            if ex == b".git" {
                has_dot_git = true;
            }
            if ex.starts_with(b"/") {
                absolute.push(ex.clone());
            } else {
                relative.push(ex.clone());
            }
        }

        let min_rel_len = relative.iter().map(|e| e.len()).min().unwrap_or(usize::MAX);
        let min_abs_len = absolute.iter().map(|e| e.len()).min().unwrap_or(usize::MAX);

        Self {
            relative,
            absolute,
            min_rel_len,
            min_abs_len,
            has_dot_git,
        }
    }

    #[inline(always)]
    pub fn is_excluded(&self, path_bytes: &[u8], name_bytes: &[u8]) -> bool {
        // Fast 4-byte check for .git
        if self.has_dot_git
            && name_bytes.len() == 4
            && u32::from_ne_bytes([name_bytes[0], name_bytes[1], name_bytes[2], name_bytes[3]])
                == DOT_GIT_U32
        {
            return true;
        }

        // Relative name match
        if name_bytes.len() >= self.min_rel_len {
            for rel in &self.relative {
                if name_bytes == rel.as_slice() {
                    return true;
                }
            }
        }

        // Absolute path match
        if path_bytes.len() >= self.min_abs_len {
            for abs in &self.absolute {
                let abs_slice = abs.as_slice();
                if path_bytes == abs_slice {
                    return true;
                }
                if path_bytes.starts_with(abs_slice)
                    && path_bytes.get(abs_slice.len()) == Some(&b'/')
                {
                    return true;
                }
            }
        }

        // Segment match across full path
        if !self.relative.is_empty() && path_bytes.len() >= self.min_rel_len {
            for rel in &self.relative {
                if has_path_segment(path_bytes, rel.as_slice()) {
                    return true;
                }
            }
        }

        false
    }
}

#[inline]
fn has_path_segment(path: &[u8], segment: &[u8]) -> bool {
    if path.len() < segment.len() {
        return false;
    }
    let mut i = 0;
    while i + segment.len() <= path.len() {
        let at_start = i == 0 || path[i - 1] == b'/';
        let at_end = i + segment.len() == path.len() || path[i + segment.len()] == b'/';
        if at_start && at_end && &path[i..i + segment.len()] == segment {
            return true;
        }
        match path[i..].iter().position(|&b| b == b'/') {
            Some(pos) => i += pos + 1,
            None => break,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_nul_equivalence() {
        for len in 0..=256 {
            let mut data = vec![b'a'; len];
            assert_eq!(find_nul(&data), len);

            if len > 0 {
                data[len - 1] = 0;
                assert_eq!(find_nul(&data), len - 1);

                data[0] = 0;
                assert_eq!(find_nul(&data), 0);

                if len > 5 {
                    data[0] = b'a';
                    data[len - 1] = b'a';
                    data[len / 2] = 0;
                    assert_eq!(find_nul(&data), len / 2);
                }
            }
        }
    }

    #[test]
    fn test_fast_exclusion_matcher() {
        let excludes = vec![b"/proc".to_vec(), b"/sys".to_vec(), b".git".to_vec()];
        let matcher = FastExclusionMatcher::new(&excludes);

        assert!(matcher.is_excluded(b"/proc", b"proc"));
        assert!(matcher.is_excluded(b"/proc/1/net", b"net"));
        assert!(matcher.is_excluded(b"/home/user/.git", b".git"));
        assert!(matcher.is_excluded(b"/home/user/.git/objects", b"objects"));
        assert!(!matcher.is_excluded(b"/home/user/project", b"project"));
    }
}
