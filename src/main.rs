use dscan::cli::CliOptions;
use dscan::scanner::{print_report, run_scan};
use dscan::ui::print_header;

fn main() {
    let options = match CliOptions::parse() {
        Some(opts) => opts,
        None => return,
    };

    print_header(&options.target_path, options.threads, &options.excludes);

    match run_scan(&options) {
        Ok(result) => {
            print_report(&result);
        }
        Err(e) => {
            eprintln!("\x1b[1m\x1b[31m❌ Error scanning path:\x1b[0m {}", e);
        }
    }
}
