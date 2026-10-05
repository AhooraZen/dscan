use std::ffi::c_void;
use std::path::Path;

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

pub const ERROR_NO_MORE_FILES: u32 = 38;
pub const ERROR_MORE_DATA: u32 = 234;

pub const STD_OUTPUT_HANDLE: u32 = 0xfffffff5;
pub const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

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
}

/// Open a directory handle for raw buffer enumeration using FILE_FLAG_BACKUP_SEMANTICS.
pub fn open_dir(path: &Path) -> Option<RawHandle> {
    use std::os::windows::ffi::OsStrExt;
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);

    // SAFETY: wide is a valid null-terminated UTF-16 slice.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
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
