use dscan_core::ScanResult;
use dscan_core::format_bytes;
use std::io::{IsTerminal, Write, stdout};
use std::time::Duration;

pub const C_RESET: &str = "\x1b[0m";
pub const C_BOLD: &str = "\x1b[1m";
pub const C_DIM: &str = "\x1b[2m";

pub const C_CYAN: &str = "\x1b[38;2;68;210;255m";
pub const C_BLUE: &str = "\x1b[38;2;90;150;255m";
pub const C_GREEN: &str = "\x1b[38;2;80;250;123m";
pub const C_PURPLE: &str = "\x1b[38;2;189;147;249m";
pub const C_YELLOW: &str = "\x1b[38;2;241;250;140m";
pub const C_GRAY: &str = "\x1b[38;2;98;114;164m";
pub const C_WHITE: &str = "\x1b[38;2;248;248;242m";
pub const C_RED: &str = "\x1b[38;2;255;85;85m";

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub reset: &'static str,
    pub bold: &'static str,
    pub dim: &'static str,
    pub cyan: &'static str,
    pub blue: &'static str,
    pub green: &'static str,
    pub purple: &'static str,
    pub yellow: &'static str,
    pub gray: &'static str,
    pub white: &'static str,
}

impl Theme {
    pub fn current() -> Self {
        if stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
            Self {
                reset: C_RESET,
                bold: C_BOLD,
                dim: C_DIM,
                cyan: C_CYAN,
                blue: C_BLUE,
                green: C_GREEN,
                purple: C_PURPLE,
                yellow: C_YELLOW,
                gray: C_GRAY,
                white: C_WHITE,
            }
        } else {
            Self {
                reset: "",
                bold: "",
                dim: "",
                cyan: "",
                blue: "",
                green: "",
                purple: "",
                yellow: "",
                gray: "",
                white: "",
            }
        }
    }
}

#[cfg(unix)]
#[repr(C)]
struct Winsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

#[cfg(unix)]
const TIOCGWINSZ: u64 = 0x5413;

#[cfg(unix)]
pub fn get_terminal_width() -> usize {
    static CACHED_WIDTH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    static TICK_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    let count = TICK_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if !count.is_multiple_of(16) {
        let cached = CACHED_WIDTH.load(std::sync::atomic::Ordering::Relaxed);
        if cached > 0 {
            return cached;
        }
    }

    let mut ws = Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: ws is a valid Winsize struct passed to standard TIOCGWINSZ ioctl on stdout fd (1).
    let ret = unsafe { dscan_core::sys::ioctl(1, TIOCGWINSZ, &mut ws as *mut Winsize) };
    let width = if ret == 0 && ws.ws_col > 10 {
        ws.ws_col as usize
    } else {
        80
    };
    CACHED_WIDTH.store(width, std::sync::atomic::Ordering::Relaxed);
    width
}

#[cfg(windows)]
pub fn get_terminal_width() -> usize {
    static CACHED_WIDTH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    static TICK_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    let count = TICK_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if !count.is_multiple_of(16) {
        let cached = CACHED_WIDTH.load(std::sync::atomic::Ordering::Relaxed);
        if cached > 0 {
            return cached;
        }
    }

    use dscan_core::sys::{
        ConsoleScreenBufferInfo, Coord, GetConsoleScreenBufferInfo, GetStdHandle,
        STD_OUTPUT_HANDLE, SmallRect,
    };
    let mut csbi = ConsoleScreenBufferInfo {
        dw_size: Coord { x: 0, y: 0 },
        dw_cursor_position: Coord { x: 0, y: 0 },
        w_attributes: 0,
        sr_window: SmallRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        dw_maximum_window_size: Coord { x: 0, y: 0 },
    };
    // SAFETY: handle retrieved via standard GetStdHandle and passed to GetConsoleScreenBufferInfo.
    let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    let ret = unsafe { GetConsoleScreenBufferInfo(handle, &mut csbi) };
    let width = if ret != 0 && csbi.sr_window.right >= csbi.sr_window.left {
        let w = (csbi.sr_window.right - csbi.sr_window.left + 1) as usize;
        if (10..=1024).contains(&w) { w } else { 80 }
    } else {
        80
    };
    CACHED_WIDTH.store(width, std::sync::atomic::Ordering::Relaxed);
    width
}

pub fn format_count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    let rem = s.len() % 3;
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (i == rem || (i > rem && (i - rem).is_multiple_of(3))) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn format_duration(d: Duration) -> String {
    let micros = d.as_micros();
    if micros < 1_000 {
        format!("{micros}µs")
    } else if micros < 1_000_000 {
        format!("{:.2}ms", d.as_secs_f64() * 1000.0)
    } else if d.as_secs() < 60 {
        format!("{:.2}s", d.as_secs_f64())
    } else {
        let mins = d.as_secs() / 60;
        let secs = d.as_secs() % 60;
        format!("{mins}m {secs}s")
    }
}

pub fn truncate_path_tail(path: &str, max_len: usize) -> String {
    if path.len() <= max_len || max_len < 4 {
        return path.to_string();
    }
    let target_bytes = max_len - 3;
    let start_idx = path.len().saturating_sub(target_bytes);

    let safe_start = (start_idx..=path.len())
        .find(|&i| path.is_char_boundary(i))
        .unwrap_or(path.len());

    format!("...{}", &path[safe_start..])
}

pub fn make_bar(percent: f64, width: usize) -> String {
    let t = Theme::current();
    make_bar_themed(percent, width, &t)
}

pub fn make_bar_themed(percent: f64, width: usize, t: &Theme) -> String {
    if width == 0 {
        return String::new();
    }
    let clamped_pct = percent.clamp(0.0, 100.0);
    let total_sub_units = ((clamped_pct / 100.0) * (width as f64 * 8.0)).round() as usize;
    let total_sub_units = total_sub_units.min(width * 8);

    let full_blocks = total_sub_units / 8;
    let rem_units = total_sub_units % 8;
    let has_rem = rem_units > 0;

    let empty_blocks = width.saturating_sub(full_blocks + if has_rem { 1 } else { 0 });

    const SUB_BLOCKS: [&str; 8] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];

    let mut out = String::with_capacity(width * 8 + 32);
    out.push_str(t.green);
    for _ in 0..full_blocks {
        out.push('█');
    }
    if has_rem {
        out.push_str(SUB_BLOCKS[rem_units]);
    }
    out.push_str(t.reset);

    if empty_blocks > 0 {
        out.push_str(t.dim);
        out.push_str(t.gray);
        for _ in 0..empty_blocks {
            out.push('░');
        }
        out.push_str(t.reset);
    }

    out
}

pub fn print_banner() {
    let t = Theme::current();
    println!(
        " {}{C_BOLD}⚡ dscan{} {}{C_DIM}v{} — Ultra-Fast Disk Space Analyzer{}\n",
        t.green,
        t.reset,
        t.cyan,
        env!("CARGO_PKG_VERSION"),
        t.reset
    );
}

pub fn print_header(_target: &str, _threads: usize, _excludes: &[String]) {
    // Deliberately no-op to eliminate multi-line startup clutter;
    // single-line header is rendered directly in render_report.
}

pub fn render_spinner_line(
    spinner_char: &str,
    bytes: u64,
    files: u64,
    active_workers: usize,
    current_path: &str,
) {
    if !stdout().is_terminal() {
        return;
    }

    let t = Theme::current();
    let term_width = get_terminal_width().clamp(30, 160);

    if term_width < 45 {
        print!(
            "\r\x1b[2K {}{}{reset} {}{C_BOLD}{}{reset}",
            t.cyan,
            spinner_char,
            t.green,
            format_bytes(bytes),
            reset = t.reset
        );
    } else if term_width < 75 {
        print!(
            "\r\x1b[2K {}{}{reset} {}{C_BOLD}{:>9}{reset} {dim}│{reset} {}{:>6} files{reset} {dim}│{reset} {}{:>2} th{reset}",
            t.cyan,
            spinner_char,
            t.green,
            format_bytes(bytes),
            t.purple,
            format_count(files),
            t.cyan,
            active_workers,
            reset = t.reset,
            dim = t.dim
        );
    } else {
        let max_path_len = term_width.saturating_sub(46).max(8);
        let truncated = truncate_path_tail(current_path, max_path_len);

        print!(
            "\r\x1b[2K {}{}{reset} {}{C_BOLD}{:>9}{reset} {dim}│{reset} {}{:>7} files{reset} {dim}│{reset} {}{:>2} th{reset} {dim}│{reset} {dim}{}{reset}",
            t.cyan,
            spinner_char,
            t.green,
            format_bytes(bytes),
            t.purple,
            format_count(files),
            t.cyan,
            active_workers,
            truncated,
            reset = t.reset,
            dim = t.dim
        );
    }
    let _ = stdout().flush();
}

pub fn clear_spinner_line() {
    if stdout().is_terminal() {
        print!("\r\x1b[2K");
        let _ = stdout().flush();
    }
}

pub fn render_report(
    target_path: &str,
    result: &ScanResult,
    threads: usize,
    excludes: &[String],
    show_extensions: bool,
) {
    let t = Theme::current();
    let width = get_terminal_width().clamp(40, 160);

    // 1. Compact Header
    let dur_str = format_duration(result.elapsed);
    let size_str = format_bytes(result.total_bytes);
    let files_str = format_count(result.total_files);

    if width < 65 {
        let max_target = width.saturating_sub(12).max(8);
        let trunc_target = truncate_path_tail(target_path, max_target);
        println!(
            " {}{C_BOLD}⚡ dscan{} {}{C_BOLD}{}{reset}",
            t.green,
            t.reset,
            t.cyan,
            trunc_target,
            reset = t.reset
        );
        println!(
            " {}{dur_str}{reset} {dim}│{reset} {}{size_str}{reset} {dim}│{reset} {}{files_str} files{reset} {dim}│{reset} {}{threads} th{reset}\n",
            t.yellow,
            t.green,
            t.purple,
            t.cyan,
            reset = t.reset,
            dim = t.dim
        );
    } else {
        let excl_suffix = if excludes.is_empty() {
            String::new()
        } else if width > 90 {
            format!(
                " {dim}│{reset} {dim}{} excl{reset}",
                excludes.len(),
                reset = t.reset,
                dim = t.dim
            )
        } else {
            String::new()
        };

        let overhead = 12
            + 3
            + 6
            + 3
            + dur_str.len()
            + 3
            + size_str.len()
            + 3
            + files_str.len()
            + 9
            + if excl_suffix.is_empty() { 0 } else { 12 };
        let max_target = width.saturating_sub(overhead).max(12);
        let trunc_target = truncate_path_tail(target_path, max_target);

        println!(
            " {}{C_BOLD}⚡ dscan{reset}  {}{C_BOLD}{}{reset} {dim}│{reset} {}{threads} th{reset} {dim}│{reset} {}{dur_str}{reset} {dim}│{reset} {}{C_BOLD}{size_str}{reset} {dim}│{reset} {}{files_str} files{reset}{excl_suffix}\n",
            t.green,
            t.cyan,
            trunc_target,
            t.cyan,
            t.yellow,
            t.green,
            t.purple,
            reset = t.reset,
            dim = t.dim
        );
    }

    let divider = "─".repeat(width.saturating_sub(2));

    let bar_width = if width < 60 {
        8
    } else if width < 90 {
        12
    } else if width < 120 {
        16
    } else {
        20
    };

    // 2. Top Directories
    println!(
        "{}{C_BOLD}📁 Top Directories{reset} {dim}(by recursive size){reset}",
        t.cyan,
        reset = t.reset,
        dim = t.dim
    );
    println!("{dim}{divider}{reset}", dim = t.dim, reset = t.reset);

    if result.top_dirs.is_empty() {
        println!(
            "  {dim}(no subdirectories found){reset}",
            dim = t.dim,
            reset = t.reset
        );
    } else {
        let hide_pct = width < 55;
        let overhead = 2 + 10 + if hide_pct { 0 } else { 8 } + 2 + bar_width + 2;
        let avail_path = width.saturating_sub(overhead).max(8);

        for (path, size) in &result.top_dirs {
            let pct = if result.max_dir_size > 0 {
                (*size as f64 / result.max_dir_size as f64) * 100.0
            } else {
                0.0
            };
            let bar = make_bar_themed(pct, bar_width, &t);
            let p_str = path.display().to_string();
            let path_disp = truncate_path_tail(&p_str, avail_path);

            if hide_pct {
                println!(
                    "  {}{C_BOLD}{:>10}{reset}  {}  {}{}{reset}",
                    t.green,
                    format_bytes(*size),
                    bar,
                    t.cyan,
                    path_disp,
                    reset = t.reset
                );
            } else {
                println!(
                    "  {}{C_BOLD}{:>10}{reset}  {}{:>5.1}%{reset}  {}  {}{}{reset}",
                    t.green,
                    format_bytes(*size),
                    t.cyan,
                    pct,
                    bar,
                    t.cyan,
                    path_disp,
                    reset = t.reset
                );
            }
        }
    }

    // 3. Top Largest Files
    println!(
        "\n{}{C_BOLD}📄 Top Largest Files{reset}",
        t.purple,
        reset = t.reset
    );
    println!("{dim}{divider}{reset}", dim = t.dim, reset = t.reset);

    if result.top_files.is_empty() {
        println!(
            "  {dim}(no files found){reset}",
            dim = t.dim,
            reset = t.reset
        );
    } else {
        let hide_pct = width < 55;
        let overhead = 2 + 10 + if hide_pct { 0 } else { 8 } + 2 + bar_width + 2;
        let avail_path = width.saturating_sub(overhead).max(8);

        for (size, path) in &result.top_files {
            let pct = if result.max_file_size > 0 {
                (*size as f64 / result.max_file_size as f64) * 100.0
            } else {
                0.0
            };
            let bar = make_bar_themed(pct, bar_width, &t);
            let p_str = path.display().to_string();
            let path_disp = truncate_path_tail(&p_str, avail_path);

            if hide_pct {
                println!(
                    "  {}{C_BOLD}{:>10}{reset}  {}  {}{}{reset}",
                    t.yellow,
                    format_bytes(*size),
                    bar,
                    t.purple,
                    path_disp,
                    reset = t.reset
                );
            } else {
                println!(
                    "  {}{C_BOLD}{:>10}{reset}  {}{:>5.1}%{reset}  {}  {}{}{reset}",
                    t.yellow,
                    format_bytes(*size),
                    t.yellow,
                    pct,
                    bar,
                    t.purple,
                    path_disp,
                    reset = t.reset
                );
            }
        }
    }

    // 4. File Extensions Breakdown Table (--ext)
    if show_extensions {
        println!(
            "\n{}{C_BOLD}📊 Top File Extensions By Size{reset}",
            t.yellow,
            reset = t.reset
        );
        println!("{dim}{divider}{reset}", dim = t.dim, reset = t.reset);

        let ext_list = dscan_core::snapshot::build_extension_breakdown(
            &result.extension_stats,
            result.total_bytes,
            12,
        );
        if ext_list.is_empty() {
            println!(
                "  {dim}(no extension data collected){reset}",
                dim = t.dim,
                reset = t.reset
            );
        } else {
            println!(
                "  {dim}{:>3}  {:<12} {:>10}   {:>6}   {:>8}   Distribution{reset}",
                "#",
                "Extension",
                "Size",
                "Share",
                "Files",
                dim = t.dim,
                reset = t.reset
            );
            for (i, item) in ext_list.iter().enumerate() {
                let bar = make_bar_themed(item.percentage_of_total, bar_width, &t);
                println!(
                    "  {dim}{:>3}{reset}  {}{C_BOLD}{:<12}{reset} {}{C_BOLD}{:>10}{reset}   {}{:>5.1}%{reset}   {dim}{:>8}{reset}   {}",
                    i + 1,
                    t.cyan,
                    item.extension,
                    t.green,
                    format_bytes(item.total_bytes),
                    t.yellow,
                    item.percentage_of_total,
                    format_count(item.file_count),
                    bar,
                    dim = t.dim,
                    reset = t.reset
                );
            }
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_count() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1000), "1,000");
        assert_eq!(format_count(7699), "7,699");
        assert_eq!(format_count(1000000), "1,000,000");
        assert_eq!(format_count(123456789), "123,456,789");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_micros(500)), "500µs");
        assert_eq!(format_duration(Duration::from_millis(50)), "50.00ms");
        assert_eq!(format_duration(Duration::from_millis(1500)), "1.50s");
        assert_eq!(format_duration(Duration::from_secs(65)), "1m 5s");
    }

    #[test]
    fn test_truncate_path_tail_ascii() {
        assert_eq!(truncate_path_tail("short/path", 20), "short/path");
        assert_eq!(
            truncate_path_tail("a/very/long/path/to/file.txt", 15),
            ".../to/file.txt"
        );
    }

    #[test]
    fn test_truncate_path_tail_persian_arabic() {
        let path = "پروژه‌های_من/دایرکتوری/تست_فایل.txt";
        let res = truncate_path_tail(path, 20);
        assert!(res.starts_with("..."));
    }

    #[test]
    fn test_truncate_path_tail_cjk() {
        let path = "文件夹/子目录/超长大文件测试.tar.gz";
        let res = truncate_path_tail(path, 16);
        assert!(res.starts_with("..."));
    }

    #[test]
    fn test_truncate_path_tail_emoji() {
        let path = "photos/🚀_launch/🌌_nebula.jpg";
        let res = truncate_path_tail(path, 18);
        assert!(res.starts_with("..."));
    }

    #[test]
    fn test_make_bar_widths() {
        assert_eq!(make_bar(100.0, 0), "");
        let bar = make_bar(50.0, 10);
        assert!(bar.contains('█'));
        assert!(bar.contains('░'));
    }

    #[test]
    fn test_make_bar_sub_blocks() {
        let bar = make_bar(53.0, 10);
        assert!(bar.contains('█'));
    }

    #[test]
    fn test_get_terminal_width() {
        let w = get_terminal_width();
        assert!(w >= 10);
    }
}
