use crate::ui::print_banner;

#[derive(Debug, Clone)]
pub struct CliOptions {
    pub target_path: String,
    pub excludes: Vec<String>,
    pub top_limit: usize,
    pub max_depth: usize,
    pub threads: usize,
    pub follow_symlinks: bool,
    pub cross_filesystems: bool,
}

impl CliOptions {
    pub fn parse() -> Option<Self> {
        let args: Vec<String> = std::env::args().collect();
        Self::parse_from_args(&args)
    }

    pub fn parse_from_args(args: &[String]) -> Option<Self> {
        let mut target_path = ".".to_string();
        #[cfg(unix)]
        let mut custom_excludes = vec![
            "/proc".to_string(),
            "/sys".to_string(),
            "/dev".to_string(),
            "/run".to_string(),
            "/tmp".to_string(),
            ".git".to_string(),
        ];
        #[cfg(windows)]
        let mut custom_excludes = vec![
            ".git".to_string(),
            "$Recycle.Bin".to_string(),
            "System Volume Information".to_string(),
            "pagefile.sys".to_string(),
            "hiberfil.sys".to_string(),
            "dumpstack.log.sys".to_string(),
        ];
        let mut top_limit = 25;
        let mut max_depth = usize::MAX; // Unlimited by default: finds all deep culprit folders!
        let mut follow_symlinks = false;
        let mut cross_filesystems = false;

        let default_threads = std::thread::available_parallelism()
            .map(|n| (n.get() * 2).clamp(4, 64))
            .unwrap_or(16);
        let mut threads = default_threads;

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
                        let d: usize = args[i + 1].parse().unwrap_or(0);
                        max_depth = if d == 0 { usize::MAX } else { d };
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "--threads" | "-j" => {
                    if i + 1 < args.len() {
                        threads = args[i + 1].parse().unwrap_or(default_threads).max(1);
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "-L" | "--follow-symlinks" => {
                    follow_symlinks = true;
                    i += 1;
                }
                "-x" | "--cross-device" | "--all-mounts" => {
                    cross_filesystems = true;
                    i += 1;
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
                    println!(
                        "  --depth <N>           Max folder depth to display (default: unlimited, 0=all)"
                    );
                    println!(
                        "  --threads, -j <N>     Number of parallel worker threads (default: cores, max 32)"
                    );
                    println!(
                        "  -L, --follow-symlinks Follow directory symlinks (e.g. Termux ~/storage)"
                    );
                    println!("  -x, --cross-device    Scan across filesystem mount boundaries");
                    println!("  -V, --version         Show version information");
                    println!("  -h, --help            Show this help menu");
                    return None;
                }
                arg if !arg.starts_with("--") && !arg.starts_with("-") => {
                    target_path = arg.to_string();
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }

        // Target path protection: do not exclude target path or its parents if user explicitly targeted it (e.g. dscan /tmp)
        let abs_target = if target_path == "." {
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .to_string_lossy()
                .to_string()
        } else {
            target_path.clone()
        };
        let abs_path = std::path::Path::new(&abs_target);

        custom_excludes.retain(|ex| {
            let ex_p = std::path::Path::new(ex);
            // Drop exclude if target_path is within or equal to this exclude
            !abs_path.starts_with(ex_p)
        });

        Some(Self {
            target_path,
            excludes: custom_excludes,
            top_limit,
            max_depth,
            threads,
            follow_symlinks,
            cross_filesystems,
        })
    }

    pub fn to_scan_options(&self) -> dscan_core::ScanOptions {
        dscan_core::ScanOptions {
            target_path: self.target_path.clone(),
            excludes: self.excludes.clone(),
            top_limit: self.top_limit,
            max_depth: self.max_depth,
            threads: self.threads,
            follow_symlinks: self.follow_symlinks,
            cross_filesystems: self.cross_filesystems,
            collect_ext_stats: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_defaults() {
        let args = vec!["dscan".to_string()];
        let opts = CliOptions::parse_from_args(&args).expect("should parse");
        assert_eq!(opts.target_path, ".");
        assert_eq!(opts.top_limit, 25);
        assert_eq!(opts.max_depth, usize::MAX);
        assert!(opts.threads >= 1);
        assert!(opts.excludes.contains(&".git".to_string()));
        assert!(!opts.follow_symlinks);
        assert!(!opts.cross_filesystems);
    }

    #[test]
    #[cfg(unix)]
    fn test_cli_target_protection_for_tmp() {
        let args = vec!["dscan".to_string(), "/tmp".to_string()];
        let opts = CliOptions::parse_from_args(&args).expect("should parse");
        assert_eq!(opts.target_path, "/tmp");
        assert!(!opts.excludes.contains(&"/tmp".to_string()));
    }

    #[test]
    fn test_cli_flags() {
        let args = vec![
            "dscan".to_string(),
            "/data".to_string(),
            "-L".to_string(),
            "-x".to_string(),
            "--depth".to_string(),
            "5".to_string(),
            "--top".to_string(),
            "10".to_string(),
            "-j".to_string(),
            "4".to_string(),
        ];
        let opts = CliOptions::parse_from_args(&args).expect("should parse");
        assert_eq!(opts.target_path, "/data");
        assert!(opts.follow_symlinks);
        assert!(opts.cross_filesystems);
        assert_eq!(opts.max_depth, 5);
        assert_eq!(opts.top_limit, 10);
        assert_eq!(opts.threads, 4);
    }
}
