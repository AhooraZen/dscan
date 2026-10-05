use std::ffi::c_void;
use std::path::Path;
use std::sync::OnceLock;

pub type RawHandle = *mut c_void;
pub const INVALID_HANDLE_VALUE: RawHandle = -1isize as RawHandle;

pub const FILE_LIST_DIRECTORY: u32 = 0x0001;
pub const FILE_SHARE_READ: u32 = 0x00000001;
pub const FILE_SHARE_WRITE: u32 = 0x00000002;
pub const FILE_SHARE_DELETE: u32 = 0x00000004;
pub const OPEN_EXISTING: u32 = 3;
pub const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;

pub const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x00000010;
pub const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;

pub const FILE_ID_BOTH_DIRECTORY_INFO: u32 = 0xa;
pub const FILE_ID_BOTH_DIRECTORY_RESTART_INFO: u32 = 0xb;
pub const FILE_ID_BOTH_DIR_INFO_CLASS: u32 = 37;

// NT Query Directory Flags for NtQueryDirectoryFileEx
pub const SL_RESTART_SCAN: u32 = 0x00000001;
pub const SL_RETURN_SINGLE_ENTRY: u32 = 0x00000002;
pub const SL_INDEX_SPECIFIED: u32 = 0x00000004;
pub const SL_RETURN_ON_DISK_ENTRIES_ONLY: u32 = 0x00000200;
pub const SL_NO_EXTENDED_ATTRIBUTES: u32 = 0x00000800;

// Virtual Memory Flags
pub const MEM_COMMIT: u32 = 0x00001000;
pub const MEM_RESERVE: u32 = 0x00002000;
pub const MEM_RELEASE: u32 = 0x00008000;
pub const MEM_LARGE_PAGES: u32 = 0x20000000;
pub const PAGE_READWRITE: u32 = 0x04;

pub const STATUS_SUCCESS: i32 = 0;
pub const STATUS_BUFFER_OVERFLOW: i32 = 0x80000005u32 as i32;
pub const STATUS_NO_MORE_FILES: i32 = 0x80000006u32 as i32;

pub const ERROR_NO_MORE_FILES: u32 = 38;
pub const ERROR_MORE_DATA: u32 = 234;

pub const STD_OUTPUT_HANDLE: u32 = 0xfffffff5;
pub const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct IoStatusBlock {
    pub status: i32,
    pub information: usize,
}

pub type NtQueryDirectoryFileFn = unsafe extern "system" fn(
    file_handle: RawHandle,
    event: RawHandle,
    apc_routine: *mut c_void,
    apc_context: *mut c_void,
    io_status_block: *mut IoStatusBlock,
    file_information: *mut c_void,
    length: u32,
    file_information_class: u32,
    return_single_entry: u8,
    file_name: *mut c_void,
    restart_scan: u8,
) -> i32;

pub type NtQueryDirectoryFileExFn = unsafe extern "system" fn(
    file_handle: RawHandle,
    event: RawHandle,
    apc_routine: *mut c_void,
    apc_context: *mut c_void,
    io_status_block: *mut IoStatusBlock,
    file_information: *mut c_void,
    length: u32,
    file_information_class: u32,
    query_flags: u32,
    file_name: *mut c_void,
) -> i32;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FileIdBothDirInfo {
    pub next_entry_offset: u32,
    pub file_index: u32,
    pub creation_time: i64,
    pub last_access_time: i64,
    pub last_write_time: i64,
    pub change_time: i64,
    pub end_of_file: i64,
    pub allocation_size: i64,
    pub file_attributes: u32,
    pub file_name_length: u32,
    pub ea_size: u32,
    pub short_name_length: i8,
    pub short_name: [u16; 12],
    pub file_id: i64,
    pub file_name: [u16; 1],
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct ByHandleFileInformation {
    pub dw_file_attributes: u32,
    pub ft_creation_time: [u32; 2],
    pub ft_last_access_time: [u32; 2],
    pub ft_last_write_time: [u32; 2],
    pub dw_volume_serial_number: u32,
    pub n_file_size_high: u32,
    pub n_file_size_low: u32,
    pub n_number_of_links: u32,
    pub n_file_index_high: u32,
    pub n_file_index_low: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Coord {
    pub x: i16,
    pub y: i16,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SmallRect {
    pub left: i16,
    pub top: i16,
    pub right: i16,
    pub bottom: i16,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ConsoleScreenBufferInfo {
    pub dw_size: Coord,
    pub dw_cursor_position: Coord,
    pub w_attributes: u16,
    pub sr_window: SmallRect,
    pub dw_maximum_window_size: Coord,
}

unsafe extern "system" {
    pub fn CreateFileW(
        lpFileName: *const u16,
        dwDesiredAccess: u32,
        dwShareMode: u32,
        lpSecurityAttributes: *mut c_void,
        dwCreationDisposition: u32,
        dwFlagsAndAttributes: u32,
        hTemplateFile: *mut c_void,
    ) -> RawHandle;

    pub fn CloseHandle(hObject: RawHandle) -> i32;

    pub fn GetFileInformationByHandleEx(
        hFile: RawHandle,
        FileInformationClass: u32,
        lpFileInformation: *mut c_void,
        dwBufferSize: u32,
    ) -> i32;

    pub fn GetFileInformationByHandle(
        hFile: RawHandle,
        lpFileInformation: *mut ByHandleFileInformation,
    ) -> i32;

    pub fn GetLastError() -> u32;

    pub fn GetStdHandle(nStdHandle: u32) -> RawHandle;

    pub fn GetConsoleScreenBufferInfo(
        hConsoleOutput: RawHandle,
        lpConsoleScreenBufferInfo: *mut ConsoleScreenBufferInfo,
    ) -> i32;

    pub fn GetConsoleMode(hConsoleHandle: RawHandle, lpMode: *mut u32) -> i32;
    pub fn SetConsoleMode(hConsoleHandle: RawHandle, dwMode: u32) -> i32;

    pub fn GetModuleHandleA(lpModuleName: *const u8) -> RawHandle;
    pub fn GetProcAddress(hModule: RawHandle, lpProcName: *const u8) -> *mut c_void;
    pub fn SetThreadAffinityMask(hThread: RawHandle, dwThreadAffinityMask: usize) -> usize;
    pub fn GetCurrentThread() -> RawHandle;

    pub fn VirtualAlloc(
        lpAddress: *mut c_void,
        dwSize: usize,
        flAllocationType: u32,
        flProtect: u32,
    ) -> *mut c_void;

    pub fn VirtualFree(lpAddress: *mut c_void, dwSize: usize, dwFreeType: u32) -> i32;

    pub fn GetLargePageMinimum() -> usize;
}

static NT_QUERY_DIR_EX: OnceLock<Option<NtQueryDirectoryFileExFn>> = OnceLock::new();

pub fn get_nt_query_directory_file_ex() -> Option<NtQueryDirectoryFileExFn> {
    *NT_QUERY_DIR_EX.get_or_init(|| {
        // SAFETY: ntdll.dll is always mapped into all Windows processes.
        unsafe {
            let ntdll = GetModuleHandleA(c"ntdll.dll".as_ptr() as *const u8);
            if ntdll.is_null() || ntdll == INVALID_HANDLE_VALUE {
                return None;
            }
            let proc = GetProcAddress(ntdll, c"NtQueryDirectoryFileEx".as_ptr() as *const u8);
            if proc.is_null() {
                None
            } else {
                Some(std::mem::transmute::<*mut c_void, NtQueryDirectoryFileExFn>(proc))
            }
        }
    })
}

static NT_QUERY_DIR: OnceLock<Option<NtQueryDirectoryFileFn>> = OnceLock::new();

pub fn get_nt_query_directory_file() -> Option<NtQueryDirectoryFileFn> {
    *NT_QUERY_DIR.get_or_init(|| {
        // SAFETY: ntdll.dll is always mapped into all Windows processes.
        unsafe {
            let ntdll = GetModuleHandleA(c"ntdll.dll".as_ptr() as *const u8);
            if ntdll.is_null() || ntdll == INVALID_HANDLE_VALUE {
                return None;
            }
            let proc = GetProcAddress(ntdll, c"NtQueryDirectoryFile".as_ptr() as *const u8);
            if proc.is_null() {
                None
            } else {
                Some(std::mem::transmute::<*mut c_void, NtQueryDirectoryFileFn>(
                    proc,
                ))
            }
        }
    })
}

/// Issues native NtQueryDirectoryFileEx with SL_NO_EXTENDED_ATTRIBUTES.
/// Transparently falls back to NtQueryDirectoryFile and GetFileInformationByHandleEx.
///
/// # Safety
/// `handle` must be an open directory handle, and `buffer` must point to at least `len` bytes.
pub unsafe fn sys_nt_query_directory_file_fast(
    handle: RawHandle,
    buffer: *mut c_void,
    len: u32,
    restart_scan: bool,
) -> (i32, usize) {
    let mut iosb = IoStatusBlock::default();
    if let Some(nt_ex) = get_nt_query_directory_file_ex() {
        let mut flags = SL_NO_EXTENDED_ATTRIBUTES;
        if restart_scan {
            flags |= SL_RESTART_SCAN;
        }
        // SAFETY: nt_ex pointer is valid from ntdll, handle is valid, and buffer has length len.
        let status = unsafe {
            nt_ex(
                handle,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut iosb,
                buffer,
                len,
                FILE_ID_BOTH_DIR_INFO_CLASS,
                flags,
                std::ptr::null_mut(),
            )
        };
        (status, iosb.information)
    } else if let Some(nt_fn) = get_nt_query_directory_file() {
        // SAFETY: nt_fn pointer is valid from ntdll, handle is valid, and buffer has length len.
        let status = unsafe {
            nt_fn(
                handle,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut iosb,
                buffer,
                len,
                FILE_ID_BOTH_DIR_INFO_CLASS,
                0,
                std::ptr::null_mut(),
                if restart_scan { 1 } else { 0 },
            )
        };
        (status, iosb.information)
    } else {
        let class = if restart_scan {
            FILE_ID_BOTH_DIRECTORY_RESTART_INFO
        } else {
            FILE_ID_BOTH_DIRECTORY_INFO
        };
        // SAFETY: handle and buffer are valid for GetFileInformationByHandleEx.
        let ret = unsafe { GetFileInformationByHandleEx(handle, class, buffer, len) };
        if ret != 0 {
            (STATUS_SUCCESS, len as usize)
        } else {
            (STATUS_NO_MORE_FILES, 0)
        }
    }
}

/// Issues native NtQueryDirectoryFile with FileIdBothDirectoryInformation.
/// Transparently falls back to GetFileInformationByHandleEx if unavailable.
///
/// # Safety
/// `handle` must be an open directory handle, and `buffer` must point to at least `len` bytes.
#[inline(always)]
pub unsafe fn sys_nt_query_directory_file(
    handle: RawHandle,
    buffer: *mut c_void,
    len: u32,
    restart_scan: bool,
) -> (i32, usize) {
    // SAFETY: caller upholds safety contract for sys_nt_query_directory_file_fast.
    unsafe { sys_nt_query_directory_file_fast(handle, buffer, len, restart_scan) }
}

/// Open directory from a null-terminated UTF-16 pointer without allocation.
///
/// # Safety
/// `ptr` must point to a valid null-terminated UTF-16 wide string.
#[inline(always)]
pub unsafe fn open_dir_from_wide_ptr(ptr: *const u16) -> Option<RawHandle> {
    // SAFETY: caller guarantees ptr points to valid null-terminated UTF-16 wide string.
    let handle = unsafe {
        CreateFileW(
            ptr,
            FILE_LIST_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE || handle.is_null() {
        None
    } else {
        Some(handle)
    }
}

/// Encode path into a reusable UTF-16 scratch buffer with null terminator.
pub fn encode_wide_into(path: &Path, scratch: &mut Vec<u16>) -> *const u16 {
    use std::os::windows::ffi::OsStrExt;
    scratch.clear();
    scratch.extend(path.as_os_str().encode_wide());
    scratch.push(0);
    scratch.as_ptr()
}

/// Open directory reusing caller's scratch buffer to avoid heap allocation.
pub fn open_dir_scratch(path: &Path, scratch: &mut Vec<u16>) -> Option<RawHandle> {
    let ptr = encode_wide_into(path, scratch);
    // SAFETY: ptr is a valid null-terminated UTF-16 wide string pointer.
    unsafe { open_dir_from_wide_ptr(ptr) }
}

/// Open a directory handle for raw buffer enumeration using FILE_FLAG_BACKUP_SEMANTICS.
pub fn open_dir(path: &Path) -> Option<RawHandle> {
    let mut scratch = Vec::new();
    open_dir_scratch(path, &mut scratch)
}

/// Safely close a Win32 file handle.
///
/// # Safety
/// `handle` must be a valid open Win32 handle or `INVALID_HANDLE_VALUE` / null.
pub unsafe fn close_handle(handle: RawHandle) {
    if handle != INVALID_HANDLE_VALUE && !handle.is_null() {
        // SAFETY: handle is checked for non-null and INVALID_HANDLE_VALUE and guaranteed by caller to be valid.
        unsafe { CloseHandle(handle) };
    }
}

/// Retrieve the volume serial number for filesystem boundary enforcement.
///
/// # Safety
/// `handle` must be a valid open Win32 file/directory handle.
pub unsafe fn get_volume_serial_number(handle: RawHandle) -> Option<u64> {
    let mut info = ByHandleFileInformation::default();
    // SAFETY: info is a valid ByHandleFileInformation struct, and caller guarantees handle is valid.
    let ret = unsafe { GetFileInformationByHandle(handle, &mut info) };
    if ret != 0 {
        Some(info.dw_volume_serial_number as u64)
    } else {
        None
    }
}

/// Pin current thread to physical CPU core on Windows.
pub fn pin_thread_to_core(core_id: usize) -> bool {
    let num_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(64);
    let target = core_id % num_cores;
    let mask = 1usize << target;
    // SAFETY: GetCurrentThread returns pseudo-handle to calling thread.
    unsafe {
        let cur = GetCurrentThread();
        SetThreadAffinityMask(cur, mask) != 0
    }
}

/// Enable Windows Virtual Terminal Processing for neon ANSI terminal escape output.
pub fn enable_virtual_terminal_processing() {
    // SAFETY: GetStdHandle and console mode calls pass valid pointers and handles.
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle != INVALID_HANDLE_VALUE && !handle.is_null() {
            let mut mode = 0u32;
            if GetConsoleMode(handle, &mut mode) != 0 {
                let _ = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
            }
        }
    }
}
