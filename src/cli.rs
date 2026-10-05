use crate::ui::print_banner;

#[derive(Debug, Clone)]
pub struct CliOptions {
    pub target_path: String,
    pub excludes: Vec<String>,
    pub top_limit: usize,
    pub max_depth: usize,
    pub threads: usize,
}

impl CliOptions {
    pub fn parse() -> Option<Self> {
        let args: Vec<String> = std::env::args().collect();
        let mut target_path = ".".to_string();
        let mut custom_excludes = vec![
            "/proc".to_string(),
            "/sys".to_string(),
            "/dev".to_string(),
            "/run".to_string(),
            "/tmp".to_string(),
            ".git".to_string(),
        ];
        let mut top_limit = 25;
        let mut max_depth = 3;
        let mut threads = 32;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--exclude" => {
                    if i + 1 < args.len() {
                        custom_excludes.push(args[i + 1].clone());
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--top" => {
                    if i + 1 < args.len() {
                        top_limit = args[i + 1].parse().unwrap_or(25);
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--depth" => {
                    if i + 1 < args.len() {
                        max_depth = args[i + 1].parse().unwrap_or(3);
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--threads" | "-j" => {
                    if i + 1 < args.len() {
                        threads = args[i + 1].parse().unwrap_or(32).max(1);
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "-V" | "--version" => {
                    println!("dscan {}", env!("CARGO_PKG_VERSION"));
                    return None;
                }
                "-h" | "--help" => {
                    print_banner();
                    println!("\x1b[1mUsage:\x1b[0m dscan [TARGET_PATH] [OPTIONS]");
                    println!("\n\x1b[38;2;68;210;255mOptions:\x1b[0m");
                    println!(
                        "  --exclude <PATTERN>   Exclude directory or pattern (e.g. --exclude .cache)"
                    );
                    println!(
                        "  --top <N>             Show top N largest files/directories (default: 25)"
                    );
                    println!("  --depth <N>           Max folder depth to aggregate (default: 3)");
                    println!(
                        "  --threads, -j <N>     Number of parallel worker threads (default: 32)"
                    );
                    println!("  -V, --version         Show version information");
                    println!("  -h, --help            Show this help menu");
                    return None;
                }
                arg if !arg.starts_with("--") => {
                    target_path = arg.to_string();
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }

        Some(Self {
            target_path,
            excludes: custom_excludes,
            top_limit,
            max_depth,
            threads,
        })
    }
}
