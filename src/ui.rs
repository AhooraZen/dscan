use std::io::{stdout, IsTerminal, Write};
use crate::format::format_bytes;

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

const TIOCGWINSZ: i64 = 0x5413;

pub fn get_terminal_width() -> usize {
    let mut ws = Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let ret = unsafe { crate::sys::syscall(16, 1, TIOCGWINSZ, &mut ws as *mut Winsize as i64) };
    if ret == 0 && ws.ws_col > 10 {
        ws.ws_col as usize
    } else {
        80
    }
}

pub fn make_bar(percent: f64, width: usize) -> String {
    let filled = ((percent / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width.saturating_sub(filled);

    let filled_str: String = "█".repeat(filled);
    let empty_str: String = "░".repeat(empty);

    format!("{C_GREEN}{}{C_DIM}{C_BLUE}{}{C_RESET}", filled_str, empty_str)
}

pub fn print_banner() {
    println!("{C_BOLD}{C_CYAN}╭──────────────────────────────────────────────────────────────────────────────╮{C_RESET}");
    println!("{C_BOLD}{C_CYAN}│  {C_GREEN}⚡ DSCAN{C_CYAN} — {C_BLUE}Ultra-Fast Modular Kernel Disk Space Analyzer{C_CYAN}                 │{C_RESET}");
    println!("{C_BOLD}{C_CYAN}╰──────────────────────────────────────────────────────────────────────────────╯{C_RESET}");
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
        let truncated = if current_path.len() > max_path_len {
            format!("...{}", &current_path[current_path.len() - (max_path_len - 3)..])
        } else {
            current_path.to_string()
        };

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
