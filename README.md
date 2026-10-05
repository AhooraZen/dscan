# ⚡ dscan

**dscan** is an ultra-fast, multi-threaded recursive disk space analyzer written in Rust, featuring a sleek neon TUI.

Inspired by KDE Filelight's recursive aggregation model and powered by raw Linux kernel `getdents64` + `fstatat` syscalls with work-stealing parallelism, `dscan` scans hundreds of thousands of files in seconds with accurate subtree size rollups.

---

## 🚀 Features

- **Blazing Fast:** Direct Linux raw directory enumeration (`getdents64` with 128 KiB buffer) bypassing standard library overhead.
- **Accurate Recursive Size Rollup:** Accurately sums directory subtree sizes similar to Filelight/GNU `du`.
- **Work-Stealing Multi-Threading:** Distributes deep directory traversal evenly across all CPU cores.
- **Sleek Cyberpunk/Neon TUI:** Clean terminal UI with live spinners, dual-tone percentage bars, and distinct largest-file discovery.
- **Custom Filters & Exclusions:** Exclude paths, set max aggregation depth, and adjust top results limit.

---

## 📦 Installation

From [crates.io](https://crates.io/crates/dscan):

```bash
cargo install dscan
```

Or build from source:

```bash
git clone https://github.com/parchlinux/dscan
cd dscan
cargo build --release
sudo cp target/release/dscan /usr/local/bin/
```

---

## 💻 Usage

```bash
# Scan current directory
dscan .

# Scan root filesystem with top 30 items and exclusions
sudo dscan / --top 30 --exclude /var/lib/docker

# Scan specific user home with depth limit
dscan /home/user --depth 2 --top 15
```

### Options

| Flag | Description | Default |
| --- | --- | --- |
| `--exclude <PATTERN>` | Pattern or path to exclude (e.g. `--exclude .git`) | System defaults |
| `--top <N>` | Number of largest directories and files to display | `25` |
| `--depth <N>` | Maximum directory depth for aggregation | `3` |
| `-h`, `--help` | Show help message | — |
| `-V`, `--version` | Show version information | — |

---

## 📜 License

Licensed under either of [MIT License](LICENSE-MIT) or [Apache License 2.0](LICENSE-APACHE) at your option.
