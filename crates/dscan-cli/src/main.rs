use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dscan_cli::cli::CliOptions;
use dscan_cli::ui::{self, print_header};
use dscan_core::run_scan_with_progress;

fn main() {
    #[cfg(windows)]
    dscan_core::sys::enable_virtual_terminal_processing();

    let options = match CliOptions::parse() {
        Some(opts) => opts,
        None => return,
    };

    print_header(&options.target_path, options.threads, &options.excludes);

    let spinners = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let idx = AtomicUsize::new(0);
    let scan_opts = options.to_scan_options();

    let progress_cb = Arc::new(move |bytes, files, workers, path: &str| {
        let i = idx.fetch_add(1, Ordering::Relaxed) % spinners.len();
        ui::render_spinner_line(spinners[i], bytes, files, workers, path);
    });

    match run_scan_with_progress(&scan_opts, Some(progress_cb)) {
        Ok(result) => {
            ui::clear_spinner_line();
            ui::render_report(&result);
        }
        Err(e) => {
            ui::clear_spinner_line();
            eprintln!("\x1b[1m\x1b[31m❌ Error scanning path:\x1b[0m {}", e);
        }
    }
}
