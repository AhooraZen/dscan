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
                    && (path_bytes.get(abs_slice.len()) == Some(&b'/')
                        || path_bytes.get(abs_slice.len()) == Some(&b'\\'))
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

#[inline(always)]
fn is_separator(b: u8) -> bool {
    b == b'/' || b == b'\\'
}

#[inline]
fn has_path_segment(path: &[u8], segment: &[u8]) -> bool {
    if path.len() < segment.len() {
        return false;
    }
    let mut i = 0;
    while i + segment.len() <= path.len() {
        let at_start = i == 0 || is_separator(path[i - 1]);
        let at_end = i + segment.len() == path.len() || is_separator(path[i + segment.len()]);
        if at_start && at_end && &path[i..i + segment.len()] == segment {
            return true;
        }
        match path[i..].iter().position(|&b| is_separator(b)) {
            Some(pos) => i += pos + 1,
            None => break,
        }
    }
    false
}

/// Ultra-fast check for "." and ".." in UTF-16 without string allocation.
#[inline(always)]
pub fn is_dot_or_dotdot_utf16(name: &[u16]) -> bool {
    match name.len() {
        1 => name[0] == 0x002E,                      // '.'
        2 => name[0] == 0x002E && name[1] == 0x002E, // '..'
        _ => false,
    }
}

/// Transcode UTF-16LE slice into UTF-8 bytes in dst buffer without heap allocation.
/// Uses fast-path ASCII vectorization (SSE2 on x86_64 or scalar loop).
pub fn transcode_utf16_to_utf8(src: &[u16], dst: &mut Vec<u8>) {
    dst.clear();
    dst.reserve(src.len() * 3); // Upper bound for BMP UTF-8 expansion

    let len = src.len();
    let mut i = 0;

    #[cfg(target_arch = "x86_64")]
    {
        #[target_feature(enable = "sse2")]
        unsafe fn transcode_ascii_sse2(src: &[u16], dst: &mut Vec<u8>, idx: &mut usize) {
            use core::arch::x86_64::*;
            let len = src.len();
            // SAFETY: Caller detected SSE2, pointer arithmetic stays within bounds, and unaligned loads are handled by _mm_loadu_si128.
            unsafe {
                while *idx + 8 <= len {
                    let chunk = _mm_loadu_si128(src.as_ptr().add(*idx) as *const __m128i);
                    // Check if all upper bytes are 0 and code units < 0x80 (ASCII)
                    let high_mask = _mm_cmpgt_epi16(chunk, _mm_set1_epi16(0x007F));
                    let low_mask = _mm_cmpgt_epi16(_mm_setzero_si128(), chunk);
                    let invalid = _mm_or_si128(high_mask, low_mask);
                    if _mm_movemask_epi8(invalid) != 0 {
                        break;
                    }
                    // Pack 8 16-bit integers into 8 8-bit unsigned integers
                    let packed = _mm_packus_epi16(chunk, chunk);
                    let val = _mm_cvtsi128_si64(packed);
                    dst.extend_from_slice(&val.to_ne_bytes());
                    *idx += 8;
                }
            }
        }

        if is_x86_feature_detected!("sse2") {
            // SAFETY: Checked runtime support for SSE2.
            unsafe { transcode_ascii_sse2(src, dst, &mut i) };
        }
    }

    // Scalar fallback for remaining elements or non-ASCII characters
    while i < len {
        let u = src[i];
        if u < 0x80 {
            dst.push(u as u8);
            i += 1;
        } else {
            // Handle multi-byte UTF-16 surrogates or multi-byte UTF-8
            for c in char::decode_utf16(src[i..].iter().copied()) {
                match c {
                    Ok(ch) => {
                        let mut buf = [0u8; 4];
                        dst.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                    Err(_) => {
                        let mut buf = [0u8; 4];
                        dst.extend_from_slice(
                            char::REPLACEMENT_CHARACTER.encode_utf8(&mut buf).as_bytes(),
                        );
                    }
                }
            }
            break;
        }
    }
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

        // Windows path separator tests
        assert!(matcher.is_excluded(b"C:\\Users\\user\\.git", b".git"));
        assert!(matcher.is_excluded(b"C:\\Users\\user\\.git\\objects", b"objects"));
        assert!(!matcher.is_excluded(b"C:\\Users\\user\\project", b"project"));
    }

    #[test]
    fn test_is_dot_or_dotdot_utf16() {
        assert!(is_dot_or_dotdot_utf16(&[b'.' as u16]));
        assert!(is_dot_or_dotdot_utf16(&[b'.' as u16, b'.' as u16]));
        assert!(!is_dot_or_dotdot_utf16(&[]));
        assert!(!is_dot_or_dotdot_utf16(&[
            b'.' as u16,
            b'.' as u16,
            b'.' as u16
        ]));
        assert!(!is_dot_or_dotdot_utf16(&[
            b'.' as u16,
            b'g' as u16,
            b'i' as u16,
            b't' as u16
        ]));
        assert!(!is_dot_or_dotdot_utf16(&[b'a' as u16]));
    }

    #[test]
    fn test_transcode_utf16_to_utf8_ascii() {
        let test_cases = [
            "",
            "a",
            "file.txt",
            "MyDirectory_123",
            "0123456789abcdef",
            "a_very_long_ascii_filename_with_more_than_sixteen_characters.dat",
            "exact_multiple_8_bytes!!",
        ];

        let mut dst = Vec::new();
        for &tc in &test_cases {
            let utf16: Vec<u16> = tc.encode_utf16().collect();
            transcode_utf16_to_utf8(&utf16, &mut dst);
            assert_eq!(String::from_utf8(dst.clone()).unwrap(), tc);
            assert_eq!(dst, String::from_utf16_lossy(&utf16).as_bytes());
        }
    }

    #[test]
    fn test_transcode_utf16_to_utf8_unicode() {
        let test_cases = [
            "سلام دنیا",
            "こんにちは世界",
            "🦀🚀🔥",
            "mixed_english_فارسی_日本語_🦀",
            "Übergrößenträger",
        ];

        let mut dst = Vec::new();
        for &tc in &test_cases {
            let utf16: Vec<u16> = tc.encode_utf16().collect();
            transcode_utf16_to_utf8(&utf16, &mut dst);
            assert_eq!(String::from_utf8(dst.clone()).unwrap(), tc);
            assert_eq!(dst, String::from_utf16_lossy(&utf16).as_bytes());
        }
    }

    #[test]
    fn test_transcode_utf16_boundary_and_lossy() {
        let mut dst = Vec::new();

        // Valid surrogate pairs: rocket emoji U+1F680 and crab emoji U+1F980
        let rocket = [0xD83D, 0xDE80];
        transcode_utf16_to_utf8(&rocket, &mut dst);
        assert_eq!(String::from_utf8(dst.clone()).unwrap(), "🚀");

        let crab = [0xD83E, 0xDD80];
        transcode_utf16_to_utf8(&crab, &mut dst);
        assert_eq!(String::from_utf8(dst.clone()).unwrap(), "🦀");

        // Lone surrogate: 0xD83D
        let lone_surrogate = [0xD83D];
        transcode_utf16_to_utf8(&lone_surrogate, &mut dst);
        assert_eq!(dst, String::from_utf16_lossy(&lone_surrogate).as_bytes());

        // Lone high surrogate followed by ASCII
        let mixed_invalid = [0xD83D, b'a' as u16];
        transcode_utf16_to_utf8(&mixed_invalid, &mut dst);
        assert_eq!(dst, String::from_utf16_lossy(&mixed_invalid).as_bytes());
    }
}
