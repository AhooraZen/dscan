use crate::format::format_bytes;
use std::io::{IsTerminal, Write, stdout};

pub const C_RESET: &str = "\x1b[0m";
pub const C_BOLD: &str = "\x1b[1m";
pub const C_DIM: &str = "\x1b[2m";

pub const C_CYAN: &str = "\x1b[38;2;68;210;255m";
pub const C_BLUE: &str = "\x1b[38;2;90;150;255m";
pub const C_GREEN: &str = "\x1b[38;2;80;250;123m";
pub const C_PURPLE: &str = "\x1b[38;2;189;147;249m";
pub const C_YELLOW: &str = "\x1b[38;2;241;250;140m";

#[repr(C)]
struct Winsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

const TIOCGWINSZ: u64 = 0x5413;

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
    println!(
        "{C_BOLD}{C_CYAN}╭──────────────────────────────────────────────────────────────────────────────╮{C_RESET}"
    );
    println!(
        "{C_BOLD}{C_CYAN}│  {C_GREEN}⚡ DSCAN{C_CYAN} — {C_BLUE}Ultra-Fast Modular Kernel Disk Space Analyzer{C_CYAN}                 │{C_RESET}"
    );
    println!(
        "{C_BOLD}{C_CYAN}╰──────────────────────────────────────────────────────────────────────────────╯{C_RESET}"
    );
}

pub fn print_header(target: &str, threads: usize, excludes: &[String]) {
    print_banner();
    println!(
        " {C_BOLD}{C_BLUE}Target:{C_RESET} {C_CYAN}{}{C_RESET}  |  {C_BOLD}{C_BLUE}Threads:{C_RESET} {C_GREEN}{}{C_RESET}  |  {C_BOLD}{C_BLUE}Exclusions:{C_RESET} {C_DIM}{:?}{C_RESET}\n",
        target, threads, excludes
    );
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

    // Compact single-line format that NEVER wraps on mobile/Termux screens
    if term_width < 70 {
        let line = format!(
            "\r\x1b[2K {C_CYAN}{}{C_RESET} {C_BOLD}{C_GREEN}{:>9}{C_RESET} | {C_BLUE}Files:{C_RESET} {:>6} | {C_BLUE}Wrk:{C_RESET} {:>2}",
            spinner_char,
            format_bytes(bytes),
            files,
            active_workers
        );
        print!("{}", line);
    } else {
        let max_path_len = term_width.saturating_sub(48).max(10);
        let truncated = truncate_path_tail(current_path, max_path_len);

        print!(
            "\r\x1b[2K {C_CYAN}{}{C_RESET} {C_BOLD}{C_GREEN}{:>10}{C_RESET} | {C_BLUE}Files:{C_RESET} {:>7} | {C_BLUE}Active:{C_RESET} {:>2} | {C_DIM}{}{C_RESET}",
            spinner_char,
            format_bytes(bytes),
            files,
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_get_terminal_width() {
        let w = get_terminal_width();
        assert!(w >= 10);
    }
}
