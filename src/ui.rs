use crate::format::format_bytes;
use crate::scanner::ScanResult;
use std::io::{IsTerminal, Write, stdout};

pub const C_RESET: &str = "\x1b[0m";
pub const C_BOLD: &str = "\x1b[1m";
pub const C_DIM: &str = "\x1b[2m";

pub const C_CYAN: &str = "\x1b[38;2;68;210;255m";
pub const C_BLUE: &str = "\x1b[38;2;90;150;255m";
pub const C_GREEN: &str = "\x1b[38;2;80;250;123m";
pub const C_PURPLE: &str = "\x1b[38;2;189;147;249m";
pub const C_YELLOW: &str = "\x1b[38;2;241;250;140m";

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
    let mut ws = Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: ws is a valid Winsize struct passed to standard TIOCGWINSZ ioctl on stdout fd (1).
    let ret = unsafe { crate::sys::ioctl(1, TIOCGWINSZ, &mut ws as *mut Winsize) };
    if ret == 0 && ws.ws_col > 10 {
        ws.ws_col as usize
    } else {
        80
    }
}

#[cfg(windows)]
pub fn get_terminal_width() -> usize {
    use crate::sys::{
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
    if ret != 0 {
        let width = (csbi.sr_window.right - csbi.sr_window.left + 1) as usize;
        if width > 10 {
            return width;
        }
    }
    80
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

pub fn truncate_path_tail(path: &str, max_len: usize) -> String {
    if path.len() <= max_len || max_len < 4 {
        return path.to_string();
    }
    let target_bytes = max_len - 3;
    let start_idx = path.len().saturating_sub(target_bytes);

    // Find nearest valid UTF-8 char boundary >= start_idx
    let safe_start = (start_idx..=path.len())
        .find(|&i| path.is_char_boundary(i))
        .unwrap_or(path.len());

    format!("...{}", &path[safe_start..])
}

pub fn make_bar(percent: f64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let filled = ((percent / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width.saturating_sub(filled);

    let filled_str: String = "█".repeat(filled);
    let empty_str: String = "░".repeat(empty);

    format!(
        "{C_GREEN}{}{C_DIM}{C_BLUE}{}{C_RESET}",
        filled_str, empty_str
    )
}

pub fn print_banner() {
    let width = get_terminal_width().clamp(36, 80);

    if width < 55 {
        println!("{C_BOLD}{C_GREEN}⚡ DSCAN{C_RESET} — {C_CYAN}Disk Space Analyzer{C_RESET}");
    } else {
        let title = " ⚡ DSCAN — Ultra-Fast Disk Space Analyzer ";
        let border_len = width.saturating_sub(title.chars().count() + 2);
        let right_border = "─".repeat(border_len);

        println!(
            "{C_BOLD}{C_CYAN}╭─{C_GREEN}{}{C_CYAN}{}╮{C_RESET}",
            title, right_border
        );
        println!(
            "{C_BOLD}{C_CYAN}╰{}╯{C_RESET}",
            "─".repeat(width.saturating_sub(2))
        );
    }
}

pub fn print_header(target: &str, threads: usize, excludes: &[String]) {
    print_banner();
    let width = get_terminal_width().clamp(36, 80);

    if width < 60 {
        println!(
            " {C_BLUE}Target:{C_RESET} {C_CYAN}{}{C_RESET} • {C_BLUE}Threads:{C_RESET} {C_GREEN}{}{C_RESET} • {C_BLUE}Excl:{C_RESET} {C_DIM}{}{C_RESET}\n",
            target,
            threads,
            excludes.len()
        );
    } else {
        let excl_str = if excludes.len() <= 3 {
            format!("{:?}", excludes)
        } else {
            format!("{} patterns", excludes.len())
        };
        println!(
            " {C_BOLD}{C_BLUE}Target:{C_RESET} {C_CYAN}{}{C_RESET}  |  {C_BOLD}{C_BLUE}Threads:{C_RESET} {C_GREEN}{}{C_RESET}  |  {C_BOLD}{C_BLUE}Exclusions:{C_RESET} {C_DIM}{}{C_RESET}\n",
            target, threads, excl_str
        );
    }
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

    let term_width = get_terminal_width();

    if term_width < 45 {
        let line = format!(
            "\r\x1b[2K {C_CYAN}{}{C_RESET} {C_BOLD}{C_GREEN}{}{C_RESET}",
            spinner_char,
            format_bytes(bytes)
        );
        print!("{}", line);
    } else if term_width < 75 {
        let line = format!(
            "\r\x1b[2K {C_CYAN}{}{C_RESET} {C_BOLD}{C_GREEN}{:>9}{C_RESET} | {C_BLUE}Files:{C_RESET} {:>6} | {C_BLUE}Wrk:{C_RESET} {:>2}",
            spinner_char,
            format_bytes(bytes),
            format_count(files),
            active_workers
        );
        print!("{}", line);
    } else {
        let max_path_len = term_width.saturating_sub(48).max(8);
        let truncated = truncate_path_tail(current_path, max_path_len);

        print!(
            "\r\x1b[2K {C_CYAN}{}{C_RESET} {C_BOLD}{C_GREEN}{:>9}{C_RESET} | {C_BLUE}Files:{C_RESET} {:>7} | {C_BLUE}Active:{C_RESET} {:>2} | {C_DIM}{}{C_RESET}",
            spinner_char,
            format_bytes(bytes),
            format_count(files),
            active_workers,
            truncated
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

pub fn render_report(result: &ScanResult) {
    let width = get_terminal_width().clamp(36, 80);

    // Summary Card
    if width < 55 {
        println!("{C_BOLD}{C_GREEN}╭─ Scan Complete ───────────────╮{C_RESET}");
        println!(
            "{C_BOLD}{C_GREEN}│{C_RESET}  {C_CYAN}Time:{C_RESET}   {C_YELLOW}{:<18.2?}{C_BOLD}{C_GREEN}│{C_RESET}",
            result.elapsed
        );
        println!(
            "{C_BOLD}{C_GREEN}│{C_RESET}  {C_CYAN}Total:{C_RESET}  {C_YELLOW}{:<18}{C_BOLD}{C_GREEN}│{C_RESET}",
            format_bytes(result.total_bytes)
        );
        println!(
            "{C_BOLD}{C_GREEN}│{C_RESET}  {C_CYAN}Files:{C_RESET}  {C_YELLOW}{:<18}{C_BOLD}{C_GREEN}│{C_RESET}",
            format_count(result.total_files)
        );
        println!("{C_BOLD}{C_GREEN}╰───────────────────────────────╯{C_RESET}\n");
    } else {
        let border_fill = "─".repeat(width.saturating_sub(18));
        println!("{C_BOLD}{C_GREEN}╭─ Scan Complete {border_fill}╮{C_RESET}");
        println!(
            "{C_BOLD}{C_GREEN}│{C_RESET}  {C_CYAN}✔ {C_YELLOW}{:.2?}{C_CYAN}  │  Total: {C_YELLOW}{:<10}{C_CYAN}  │  Files: {C_YELLOW}{:<8}{C_BOLD}{C_GREEN}│{C_RESET}",
            result.elapsed,
            format_bytes(result.total_bytes),
            format_count(result.total_files)
        );
        println!(
            "{C_BOLD}{C_GREEN}╰{}╯{C_RESET}\n",
            "─".repeat(width.saturating_sub(2))
        );
    }

    let divider = "─".repeat(width.saturating_sub(2));

    // Dynamic bar width
    let bar_width = if width < 50 {
        6
    } else if width < 65 {
        8
    } else if width < 80 {
        12
    } else {
        16
    };

    // Top Directories
    println!("{C_BOLD}{C_CYAN}📁 Top Directories By Recursive Size:{C_RESET}");
    println!("{C_DIM}{divider}{C_RESET}");

    if result.top_dirs.is_empty() {
        println!("  {C_DIM}(no subdirectories found){C_RESET}");
    } else {
        let overhead = 10 + 2 + bar_width + 3; // size(10) + space(2) + bar + space(3)
        let avail_path = width.saturating_sub(overhead).max(8);

        for (path, size) in &result.top_dirs {
            let pct = (*size as f64 / result.max_dir_size as f64) * 100.0;
            let bar = make_bar(pct, bar_width);
            let p_str = path.display().to_string();
            let path_disp = truncate_path_tail(&p_str, avail_path);

            println!(
                "  {C_BOLD}{C_GREEN}{:>10}{C_RESET} {} {C_CYAN}{}{C_RESET}",
                format_bytes(*size),
                bar,
                path_disp
            );
        }
    }

    // Top Largest Files
    println!("\n{C_BOLD}{C_PURPLE}📄 Top Largest Files:{C_RESET}");
    println!("{C_DIM}{divider}{C_RESET}");

    if result.top_files.is_empty() {
        println!("  {C_DIM}(no files found){C_RESET}");
    } else {
        let overhead = 10 + 2 + bar_width + 3;
        let avail_path = width.saturating_sub(overhead).max(8);

        for (size, path) in &result.top_files {
            let pct = (*size as f64 / result.max_file_size as f64) * 100.0;
            let bar = make_bar(pct, bar_width);
            let p_str = path.display().to_string();
            let path_disp = truncate_path_tail(&p_str, avail_path);

            println!(
                "  {C_BOLD}{C_YELLOW}{:>10}{C_RESET} {} {C_PURPLE}{}{C_RESET}",
                format_bytes(*size),
                bar,
                path_disp
            );
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
    fn test_get_terminal_width() {
        let w = get_terminal_width();
        assert!(w >= 10);
    }
}
