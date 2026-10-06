# Plan 003: Multi-Platform Package Manager Distribution (AUR, Homebrew, Scoop)

> **Execution context**: This plan specifies the full distribution strategy, packaging manifests, repository setup, release automation (CI/CD), and operational prerequisites for distributing `dscan` (CLI) and `dscan-gui` (desktop visualizer) across Arch Linux (AUR), macOS/Linux (Homebrew), and Windows (Scoop).

## Status
- **Priority**: P1
- **Effort**: M
- **Risk**: LOW (strictly additive distribution infrastructure; zero changes to scanning engine or core business logic)
- **Target Version Baseline**: `v0.6.0`+
- **Upstream Release Assets**:
  - `dscan-linux-x86_64.tar.gz` (contains `dscan` binary)
  - `dscan-linux-aarch64.tar.gz` (contains `dscan` binary)
  - `dscan-windows-x64.zip` (contains `dscan.exe` binary)
  - `dscan-gui-linux-x86_64.tar.gz` (contains `dscan-gui` binary)
  - `dscan-gui-windows-x64.zip` (contains `dscan-gui.exe` binary)
  - `dscan-android-arm64.apk`
  - `SHA256SUMS.txt` (contains unified hashes for all release artifacts)

---

## 1. AUR (Arch User Repository)

### 1.1 Architecture & Package Selection
Three packages serve the Arch Linux user base:
1. **`dscan-bin`** (Recommended for most users): Fast binary distribution downloading precompiled releases (`x86_64` and `aarch64`). Requires zero build dependencies and installs instantly without requiring the Rust toolchain.
2. **`dscan`** (Source package): Builds from source using `cargo build --release --locked -p dscan`. Adheres to Arch Linux guidelines for users who build packages locally.
3. **`dscan-gui-bin`**: Precompiled desktop visualizer binary (`x86_64`) with GPUI runtime dependencies and `.desktop` launcher.

---

### 1.2 Exact File Contents

#### A. `dscan-bin` — Precompiled CLI Binary (`PKGBUILD`)
```bash
# Maintainer: AhooraZen <ahoora935137@gmail.com>
pkgname=dscan-bin
_pkgname=dscan
pkgver=0.6.0
pkgrel=1
pkgdesc="Fast, zero-dependency multi-threaded disk usage analyzer (precompiled binary)"
arch=('x86_64' 'aarch64')
url="https://github.com/AhooraZen/dscan"
license=('MIT' 'Apache-2.0')
depends=('glibc')
provides=("${_pkgname}")
conflicts=("${_pkgname}")

source_x86_64=("${_pkgname}-${pkgver}-x86_64.tar.gz::${url}/releases/download/v${pkgver}/${_pkgname}-linux-x86_64.tar.gz")
source_aarch64=("${_pkgname}-${pkgver}-aarch64.tar.gz::${url}/releases/download/v${pkgver}/${_pkgname}-linux-aarch64.tar.gz")
source=("LICENSE-MIT::https://raw.githubusercontent.com/AhooraZen/dscan/v${pkgver}/LICENSE-MIT"
        "LICENSE-APACHE::https://raw.githubusercontent.com/AhooraZen/dscan/v${pkgver}/LICENSE-APACHE")

sha256sums=('SKIP'
            'SKIP')
# Populate with exact release hashes per architecture
sha256sums_x86_64=('REPLACE_WITH_SHA256_LINUX_X86_64')
sha256sums_aarch64=('REPLACE_WITH_SHA256_LINUX_AARCH64')

package() {
    install -Dm755 "${srcdir}/${_pkgname}" "${pkgdir}/usr/bin/${_pkgname}"
    install -Dm644 "${srcdir}/LICENSE-MIT" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE-MIT"
    install -Dm644 "${srcdir}/LICENSE-APACHE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE-APACHE"
}
```

#### B. `dscan-gui-bin` — Precompiled Desktop GUI Binary (`PKGBUILD`)
```bash
# Maintainer: AhooraZen <ahoora935137@gmail.com>
pkgname=dscan-gui-bin
_pkgname=dscan-gui
pkgver=0.6.0
pkgrel=1
pkgdesc="Native GPU cushion treemap disk usage visualizer (precompiled binary)"
arch=('x86_64')
url="https://github.com/AhooraZen/dscan"
license=('MIT' 'Apache-2.0')
depends=('alsa-lib' 'fontconfig' 'wayland' 'libx11' 'libxkbcommon' 'vulkan-icd-loader')
optdepends=(
    'vulkan-radeon: Vulkan support for AMD graphics'
    'vulkan-intel: Vulkan support for Intel graphics'
    'nvidia-utils: Vulkan support for NVIDIA graphics'
)
provides=("${_pkgname}")
conflicts=("${_pkgname}")

source_x86_64=("${_pkgname}-${pkgver}-x86_64.tar.gz::${url}/releases/download/v${pkgver}/${_pkgname}-linux-x86_64.tar.gz")
source=("${_pkgname}.desktop"
        "LICENSE-MIT::https://raw.githubusercontent.com/AhooraZen/dscan/v${pkgver}/LICENSE-MIT"
        "LICENSE-APACHE::https://raw.githubusercontent.com/AhooraZen/dscan/v${pkgver}/LICENSE-APACHE")

sha256sums=('SKIP'
            'SKIP'
            'SKIP')
sha256sums_x86_64=('REPLACE_WITH_SHA256_GUI_LINUX_X86_64')

package() {
    install -Dm755 "${srcdir}/${_pkgname}" "${pkgdir}/usr/bin/${_pkgname}"
    install -Dm644 "${srcdir}/${_pkgname}.desktop" "${pkgdir}/usr/share/applications/${_pkgname}.desktop"
    install -Dm644 "${srcdir}/LICENSE-MIT" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE-MIT"
    install -Dm644 "${srcdir}/LICENSE-APACHE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE-APACHE"
}
```

#### C. `dscan-gui.desktop` — Desktop Entry
```ini
[Desktop Entry]
Name=dscan GUI
GenericName=Disk Space Analyzer
Comment=Native GPU-accelerated WinDirStat cushion treemap visualizer
Exec=dscan-gui
Terminal=false
Type=Application
Icon=utilities-system-monitor
Categories=System;Filesystem;Utility;
Keywords=disk;usage;analyzer;treemap;windirstat;storage;du;
StartupNotify=true
```

#### D. `dscan` — Source Build (`PKGBUILD`)
```bash
# Maintainer: AhooraZen <ahoora935137@gmail.com>
pkgname=dscan
pkgver=0.6.0
pkgrel=1
pkgdesc="Fast, zero-dependency multi-threaded disk usage analyzer for Linux"
arch=('x86_64' 'aarch64')
url="https://github.com/AhooraZen/dscan"
license=('MIT' 'Apache-2.0')
depends=('glibc' 'gcc-libs')
makedepends=('cargo')
source=("${pkgname}-${pkgver}.tar.gz::${url}/archive/refs/tags/v${pkgver}.tar.gz")
sha256sums=('REPLACE_WITH_SHA256_SOURCE_TARBALL')

prepare() {
    cd "${srcdir}/${pkgname}-${pkgver}"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "${srcdir}/${pkgname}-${pkgver}"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --frozen --release -p dscan
}

check() {
    cd "${srcdir}/${pkgname}-${pkgver}"
    export RUSTUP_TOOLCHAIN=stable
    cargo test --frozen --release -p dscan-core -p dscan
}

package() {
    cd "${srcdir}/${pkgname}-${pkgver}"
    install -Dm755 "target/release/${pkgname}" "${pkgdir}/usr/bin/${pkgname}"
    install -Dm644 LICENSE-MIT "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE-MIT"
    install -Dm644 LICENSE-APACHE "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE-APACHE"
}
```

---

### 1.3 Repository Setup & Initial Submission
1. **Create an AUR Account**:
   - Register at [aur.archlinux.org](https://aur.archlinux.org/).
   - Add your SSH public key (`~/.ssh/aur.pub`) in Account Settings.
2. **Configure SSH for AUR**:
   Add to `~/.ssh/config`:
   ```ssh
   Host aur.archlinux.org
       User aur
       IdentityFile ~/.ssh/aur
   ```
3. **Initialize the Remote Repositories**:
   ```bash
   git clone ssh://aur@aur.archlinux.org/dscan-bin.git ~/Git-Clones/aur-dscan-bin
   cd ~/Git-Clones/aur-dscan-bin
   ```
4. **Generate `.SRCINFO` and Validate**:
   Arch AUR requires `.SRCINFO` to be committed. The AUR server parses `.SRCINFO` directly and ignores `PKGBUILD` for web metadata:
   ```bash
   # Generate .SRCINFO
   makepkg --printsrcinfo > .SRCINFO

   # Test local build in clean container or chroot
   makepkg -sfc

   # Verify package with namcap linter
   namcap PKGBUILD
   namcap dscan-bin-*.pkg.tar.zst
   ```
5. **Publish the First Commit**:
   ```bash
   git add PKGBUILD .SRCINFO
   git commit -m "feat: initial release v0.6.0"
   git push -u origin master
   ```
   Repeat the same process for `dscan-gui-bin` (adding `dscan-gui.desktop`).

---

### 1.4 CI Automation for Version Bumps
To publish updates to the AUR automatically on new GitHub releases, use SSH deploy keys stored in GitHub Secrets.

#### GitHub Secret Setup
- Add repository secret `AUR_SSH_PRIVATE_KEY` (private key matching the public key on AUR).

#### GitHub Actions Workflow Job (Reusable AUR Deploy Step)
Add this job to `.github/workflows/release.yml`:
```yaml
  publish-aur:
    name: Publish to AUR
    needs: [publish-github-release]
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Setup SSH Keys for AUR
        run: |
          mkdir -p ~/.ssh
          echo "${{ secrets.AUR_SSH_PRIVATE_KEY }}" > ~/.ssh/aur_key
          chmod 600 ~/.ssh/aur_key
          ssh-keyscan -H aur.archlinux.org >> ~/.ssh/known_hosts

      - name: Update and Push dscan-bin
        env:
          GIT_SSH_COMMAND: "ssh -i ~/.ssh/aur_key"
        run: |
          VERSION="${GITHUB_REF_NAME#v}"
          
          # Fetch checksums from release asset
          curl -sSL "https://github.com/AhooraZen/dscan/releases/download/${GITHUB_REF_NAME}/SHA256SUMS.txt" -o SHA256SUMS.txt
          SHA_X86_64=$(grep "dscan-linux-x86_64.tar.gz" SHA256SUMS.txt | awk '{print $1}')
          SHA_AARCH64=$(grep "dscan-linux-aarch64.tar.gz" SHA256SUMS.txt | awk '{print $1}')

          git clone ssh://aur@aur.archlinux.org/dscan-bin.git /tmp/aur-dscan-bin
          cd /tmp/aur-dscan-bin

          # Update version and checksums
          sed -i "s/^pkgver=.*/pkgver=${VERSION}/" PKGBUILD
          sed -i "s/^pkgrel=.*/pkgrel=1/" PKGBUILD
          sed -i "s/^sha256sums_x86_64=.*/sha256sums_x86_64=('${SHA_X86_64}')/" PKGBUILD
          sed -i "s/^sha256sums_aarch64=.*/sha256sums_aarch64=('${SHA_AARCH64}')/" PKGBUILD

          # Generate .SRCINFO using docker to avoid installing Arch tooling on Ubuntu runner
          docker run --rm -v "$(pwd):/pkg" -w /pkg archlinux:base bash -c "pacman -Sy --noconfirm pacman-contrib && makepkg --printsrcinfo > .SRCINFO"

          git config user.name "AhooraZen"
          git config user.email "ahoora935137@gmail.com"
          git add PKGBUILD .SRCINFO
          if git commit -m "chore(release): bump dscan-bin to ${VERSION}"; then
            git push origin master
          fi
```

---

### 1.5 Gotchas & Prerequisites (Arch/AUR)
- **Strict License Installation**: Arch packaging standards mandate that any MIT or Apache-2.0 software must install licenses to `/usr/share/licenses/$pkgname/`. Omitting this will fail `namcap` audits.
- **Out-of-Sync `.SRCINFO`**: Never push `PKGBUILD` without updating `.SRCINFO`. The AUR web UI and AUR helpers (`yay`, `paru`) read `.SRCINFO` metadata, not the `PKGBUILD`.
- **Targeting Arch Linux ARM**: `dscan-linux-aarch64.tar.gz` is built in `release.yml`. Using `arch=('x86_64' 'aarch64')` with `source_x86_64` and `source_aarch64` enables seamless compatibility with Arch Linux ARM (ALARM) on Raspberry Pi and Asahi Linux.
- **Mold Linker on Arch**: When building `dscan-gui` from source, Arch users must have Vulkan and Wayland headers. Precompiled `-bin` packages completely avoid this build friction for end users.

---

## 2. Homebrew (macOS / Linux)

### 2.1 Architecture & Tap Design
Homebrew packages are distributed via an official custom tap repository:
- **Repository Name**: `https://github.com/AhooraZen/homebrew-dscan`
- **Naming Rule**: Homebrew requires tap repositories to follow the `homebrew-<name>` convention. Users install using `brew tap AhooraZen/dscan` or directly `brew install AhooraZen/dscan/dscan`.
- **Formula Path**: `Formula/dscan.rb`

### 2.2 macOS Status & Handling
> **Current Engine Constraint**: `dscan-core` is currently designed with direct Linux kernel syscalls (`SYS_GETDENTS64`, `statx`, `open_dir_at2`) and Windows NT kernel APIs (`ntdll.dll`). There is no macOS backend yet (which requires Darwin `getattrlistbulk` or `getdirentries64`). Furthermore, the CI pipeline currently only targets Linux and Windows.
>
> **Formula Strategy**: The formula distributes precompiled Linux binaries (`x86_64` and `aarch64`). For macOS requests, the formula cleanly enforces `depends_on :linux`, preventing cryptic compilation crashes and providing an actionable message pointing to the GitHub issue tracking macOS development.

---

### 2.3 Exact File Contents

#### `Formula/dscan.rb`
```ruby
class Dscan < Formula
  desc "Fast, zero-dependency multi-threaded disk usage analyzer"
  homepage "https://github.com/AhooraZen/dscan"
  version "0.6.0"
  license any_of: ["MIT", "Apache-2.0"]

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/AhooraZen/dscan/releases/download/v0.6.0/dscan-linux-x86_64.tar.gz"
      sha256 "REPLACE_WITH_SHA256_LINUX_X86_64"
    elsif Hardware::CPU.arm?
      url "https://github.com/AhooraZen/dscan/releases/download/v0.6.0/dscan-linux-aarch64.tar.gz"
      sha256 "REPLACE_WITH_SHA256_LINUX_AARCH64"
    end
  end

  on_macos do
    # Native macOS support (getattrlistbulk Darwin engine) is currently in development.
    # Track progress or contribute at: https://github.com/AhooraZen/dscan/issues
    depends_on :linux
  end

  def install
    bin.install "dscan"
  end

  test do
    # Verify version output
    assert_match "dscan", shell_output("#{bin}/dscan --version")

    # Verify basic scanning execution on test directory
    test_dir = testpath/"test_scan"
    test_dir.mkpath
    (test_dir/"sample.txt").write("dscan disk analyzer test file")

    output = shell_output("#{bin}/dscan #{test_dir} --top 5")
    assert_match "Total Space Scanned", output
  end
end
```

#### `README.md` (inside `homebrew-dscan` repository)
```markdown
# homebrew-dscan

Official Homebrew tap for [dscan](https://github.com/AhooraZen/dscan) — Fast, zero-dependency multi-threaded disk usage analyzer.

## Installation

```bash
brew tap AhooraZen/dscan
brew install dscan
```

Or install in a single step:

```bash
brew install AhooraZen/dscan/dscan
```

## Updating

```bash
brew update
brew upgrade dscan
```
```

---

### 2.4 Tap Repository Setup
1. **Create the GitHub Repository**:
   ```bash
   gh repo create AhooraZen/homebrew-dscan --public \
     --description "Official Homebrew tap for dscan disk usage analyzer"
   ```
2. **Populate Structure**:
   ```bash
   git clone https://github.com/AhooraZen/homebrew-dscan.git
   cd homebrew-dscan
   mkdir -p Formula
   cp /path/to/dscan.rb Formula/dscan.rb
   git add Formula/dscan.rb README.md
   git commit -m "feat: initial dscan formula v0.6.0"
   git push -u origin main
   ```
3. **Verify Locally with Homebrew Audit**:
   ```bash
   brew audit --strict --online Formula/dscan.rb
   brew test Formula/dscan.rb
   ```

---

### 2.5 CI Automation for Version Bumps
When a release tag is pushed to `AhooraZen/dscan`, GitHub Actions automatically updates the formula in `AhooraZen/homebrew-dscan`.

#### GitHub Secret Setup
Create a Personal Access Token (classic with `public_repo` or fine-grained with Contents: Write on `homebrew-dscan`) and store it as `HOMEBREW_TAP_TOKEN` in `AhooraZen/dscan` Secrets.

#### GitHub Actions Workflow Job
Add this job to `.github/workflows/release.yml`:
```yaml
  publish-homebrew:
    name: Publish to Homebrew Tap
    needs: [publish-github-release]
    runs-on: ubuntu-latest
    steps:
      - name: Checkout dscan
        uses: actions/checkout@v4

      - name: Update Formula in homebrew-dscan
        env:
          TAP_TOKEN: ${{ secrets.HOMEBREW_TAP_TOKEN }}
        run: |
          VERSION="${GITHUB_REF_NAME#v}"
          
          # Fetch checksums
          curl -sSL "https://github.com/AhooraZen/dscan/releases/download/${GITHUB_REF_NAME}/SHA256SUMS.txt" -o SHA256SUMS.txt
          SHA_X86_64=$(grep "dscan-linux-x86_64.tar.gz" SHA256SUMS.txt | awk '{print $1}')
          SHA_AARCH64=$(grep "dscan-linux-aarch64.tar.gz" SHA256SUMS.txt | awk '{print $1}')

          # Clone tap repository
          git clone "https://x-access-token:${TAP_TOKEN}@github.com/AhooraZen/homebrew-dscan.git" /tmp/homebrew-dscan
          cd /tmp/homebrew-dscan/Formula

          # Generate updated formula
          cat <<EOF > dscan.rb
          class Dscan < Formula
            desc "Fast, zero-dependency multi-threaded disk usage analyzer"
            homepage "https://github.com/AhooraZen/dscan"
            version "${VERSION}"
            license any_of: ["MIT", "Apache-2.0"]

            on_linux do
              if Hardware::CPU.intel?
                url "https://github.com/AhooraZen/dscan/releases/download/v${VERSION}/dscan-linux-x86_64.tar.gz"
                sha256 "${SHA_X86_64}"
              elsif Hardware::CPU.arm?
                url "https://github.com/AhooraZen/dscan/releases/download/v${VERSION}/dscan-linux-aarch64.tar.gz"
                sha256 "${SHA_AARCH64}"
              end
            end

            on_macos do
              # Native macOS support is in development.
              depends_on :linux
            end

            def install
              bin.install "dscan"
            end

            test do
              assert_match "dscan", shell_output("#{bin}/dscan --version")
              test_dir = testpath/"test_scan"
              test_dir.mkpath
              (test_dir/"sample.txt").write("dscan test")
              output = shell_output("#{bin}/dscan #{test_dir} --top 5")
              assert_match "Total Space Scanned", output
            end
          end
          EOF

          # Remove heredoc leading indentation
          sed -i 's/^[ ]\{10\}//' dscan.rb

          cd /tmp/homebrew-dscan
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git add Formula/dscan.rb
          if git commit -m "chore: bump dscan formula to v${VERSION}"; then
            git push origin main
          fi
```

---

### 2.6 Gotchas & Prerequisites (Homebrew)
- **Strict Class Name Matching**: Homebrew requires that `Formula/dscan.rb` defines `class Dscan < Formula`. A mismatch causes syntax parsing failure.
- **Unpack Behavior**: `dscan-linux-x86_64.tar.gz` contains only the `dscan` binary in the archive root. Homebrew automatically untars the archive into a flat directory; `bin.install "dscan"` reliably copies it to `/home/linuxbrew/.linuxbrew/bin/dscan`.
- **Precompiled vs Source**: Homebrew core prefers source builds, but taps (like `homebrew-dscan`) support binary distributions seamlessly (`url` pointing directly to `.tar.gz`).
- **macOS Future Expansion**: Once Darwin kernel scanning (`getattrlistbulk`) lands, update `on_macos` block to:
  ```ruby
  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/AhooraZen/dscan/releases/download/v#{version}/dscan-macos-aarch64.tar.gz"
      sha256 "..."
    elsif Hardware::CPU.intel?
      url "https://github.com/AhooraZen/dscan/releases/download/v#{version}/dscan-macos-x86_64.tar.gz"
      sha256 "..."
    end
  end
  ```

---

## 3. Scoop (Windows)

### 3.1 Architecture & Bucket Design
Scoop is the premier command-line installer for Windows developers.
- **Custom Bucket**: `https://github.com/AhooraZen/scoop-dscan`
- **Installation Command**:
  ```powershell
  scoop bucket add dscan https://github.com/AhooraZen/scoop-dscan
  scoop install dscan
  scoop install dscan-gui
  ```
- **Bucket Directory Layout**:
  ```text
  scoop-dscan/
  ├── bucket/
  │   ├── dscan.json       # CLI tool
  │   └── dscan-gui.json   # Desktop GUI visualizer
  ├── README.md
  └── .github/
      └── workflows/
          └── auto-update.yml
  ```
- **Official Scoop Ingestion Path**:
  - `dscan` CLI qualifies for official inclusion in `ScoopInstaller/Main`.
  - `dscan-gui` qualifies for official inclusion in `ScoopInstaller/Extras` (for GUI applications).

---

### 3.2 Exact File Contents

#### A. `bucket/dscan.json` — CLI Tool Manifest
```json
{
  "version": "0.6.0",
  "description": "Fast, zero-dependency multi-threaded disk usage analyzer for Linux and Windows",
  "homepage": "https://github.com/AhooraZen/dscan",
  "license": "MIT|Apache-2.0",
  "architecture": {
    "64bit": {
      "url": "https://github.com/AhooraZen/dscan/releases/download/v0.6.0/dscan-windows-x64.zip",
      "hash": "REPLACE_WITH_SHA256_WINDOWS_X64"
    }
  },
  "bin": "dscan.exe",
  "checkver": "github",
  "autoupdate": {
    "architecture": {
      "64bit": {
        "url": "https://github.com/AhooraZen/dscan/releases/download/v$version/dscan-windows-x64.zip"
      }
    }
  }
}
```

#### B. `bucket/dscan-gui.json` — Desktop GUI Visualizer Manifest
```json
{
  "version": "0.6.0",
  "description": "Native GPU-accelerated WinDirStat cushion treemap visualizer for dscan",
  "homepage": "https://github.com/AhooraZen/dscan",
  "license": "MIT|Apache-2.0",
  "architecture": {
    "64bit": {
      "url": "https://github.com/AhooraZen/dscan/releases/download/v0.6.0/dscan-gui-windows-x64.zip",
      "hash": "REPLACE_WITH_SHA256_GUI_WINDOWS_X64"
    }
  },
  "bin": "dscan-gui.exe",
  "shortcuts": [
    [
      "dscan-gui.exe",
      "dscan GUI"
    ]
  ],
  "checkver": "github",
  "autoupdate": {
    "architecture": {
      "64bit": {
        "url": "https://github.com/AhooraZen/dscan/releases/download/v$version/dscan-gui-windows-x64.zip"
      }
    }
  }
}
```

#### C. `README.md` (inside `scoop-dscan` repository)
```markdown
# scoop-dscan

Official Scoop bucket for [dscan](https://github.com/AhooraZen/dscan) on Windows.

## Installation

Add this bucket:
```powershell
scoop bucket add dscan https://github.com/AhooraZen/scoop-dscan
```

Install CLI:
```powershell
scoop install dscan
```

Install Desktop Treemap Visualizer:
```powershell
scoop install dscan-gui
```

## Updating

```powershell
scoop update
scoop status
scoop update dscan dscan-gui
```
```

---

### 3.3 Bucket Repository Setup
1. **Create the GitHub Repository**:
   ```bash
   gh repo create AhooraZen/scoop-dscan --public \
     --description "Official Scoop bucket for dscan and dscan-gui on Windows"
   ```
2. **Populate the Manifests**:
   ```bash
   git clone https://github.com/AhooraZen/scoop-dscan.git
   cd scoop-dscan
   mkdir -p bucket
   # Add dscan.json and dscan-gui.json
   git add bucket/ README.md
   git commit -m "feat: initial bucket manifests v0.6.0"
   git push -u origin main
   ```
3. **Verify Locally with Scoop**:
   In PowerShell on a Windows machine or test VM:
   ```powershell
   scoop bucket add test-dscan C:\path\to\scoop-dscan
   scoop install test-dscan/dscan
   scoop test C:\path\to\scoop-dscan\bucket\dscan.json
   ```

---

### 3.4 Automation for Version Bumps
Scoop has native support for automatic updates via GitHub Actions:
- **`checkver`**: Scoop connects to GitHub Releases API (`https://api.github.com/repos/AhooraZen/dscan/releases/latest`) and automatically detects new tags (`v*`).
- **`autoupdate`**: Scoop downloads the release `.zip`, computes the SHA256 checksum, and updates the manifest.

#### Method 1: Bucket Auto-Update Workflow (`ScoopInstaller/GithubActions`)
Place this workflow in `AhooraZen/scoop-dscan/.github/workflows/auto-update.yml`:
```yaml
name: Auto Update Manifests

on:
  schedule:
    - cron: '*/30 * * * *' # Check every 30 minutes
  workflow_dispatch:

jobs:
  auto-update:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4

      - name: Run Scoop Auto Update
        uses: ScoopInstaller/GithubActions@latest
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

#### Method 2: Direct CI Dispatch from `dscan` Release Workflow
To update Scoop manifests immediately upon release (without waiting for cron), add this step to `dscan/.github/workflows/release.yml`:
```yaml
  publish-scoop:
    name: Publish to Scoop Bucket
    needs: [publish-github-release]
    runs-on: ubuntu-latest
    steps:
      - name: Update Scoop Manifests
        env:
          SCOOP_BUCKET_TOKEN: ${{ secrets.SCOOP_BUCKET_TOKEN }}
        run: |
          VERSION="${GITHUB_REF_NAME#v}"
          
          # Fetch checksums
          curl -sSL "https://github.com/AhooraZen/dscan/releases/download/${GITHUB_REF_NAME}/SHA256SUMS.txt" -o SHA256SUMS.txt
          SHA_WIN_CLI=$(grep "dscan-windows-x64.zip" SHA256SUMS.txt | awk '{print $1}')
          SHA_WIN_GUI=$(grep "dscan-gui-windows-x64.zip" SHA256SUMS.txt | awk '{print $1}')

          git clone "https://x-access-token:${SCOOP_BUCKET_TOKEN}@github.com/AhooraZen/scoop-dscan.git" /tmp/scoop-dscan
          cd /tmp/scoop-dscan/bucket

          # Update dscan.json (CLI)
          jq --arg v "$VERSION" --arg h "$SHA_WIN_CLI" \
            '.version = $v | .architecture."64bit".hash = $h | .architecture."64bit".url = "https://github.com/AhooraZen/dscan/releases/download/v" + $v + "/dscan-windows-x64.zip"' \
            dscan.json > dscan.json.tmp && mv dscan.json.tmp dscan.json

          # Update dscan-gui.json (GUI)
          jq --arg v "$VERSION" --arg h "$SHA_WIN_GUI" \
            '.version = $v | .architecture."64bit".hash = $h | .architecture."64bit".url = "https://github.com/AhooraZen/dscan/releases/download/v" + $v + "/dscan-gui-windows-x64.zip"' \
            dscan-gui.json > dscan-gui.json.tmp && mv dscan-gui.json.tmp dscan-gui.json

          cd /tmp/scoop-dscan
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git add bucket/
          if git commit -m "chore: bump manifests to v${VERSION}"; then
            git push origin main
          fi
```

---

### 3.5 Submission to Official Scoop Buckets
Once stable releases are running smoothly in `scoop-dscan`, submit upstream:
1. **Submit `dscan` to `ScoopInstaller/Main`**:
   - Fork [ScoopInstaller/Main](https://github.com/ScoopInstaller/Main).
   - Add `bucket/dscan.json`.
   - Run verification tests:
     ```powershell
     .\bin\checkver.ps1 dscan
     .\bin\test.ps1 -Manifest dscan
     ```
   - Open Pull Request.
2. **Submit `dscan-gui` to `ScoopInstaller/Extras`**:
   - Fork [ScoopInstaller/Extras](https://github.com/ScoopInstaller/Extras).
   - Add `bucket/dscan-gui.json`.
   - Run verification tests and open Pull Request.

---

### 3.6 Gotchas & Prerequisites (Scoop)
- **Zip Compression Structure**: `release.yml` produces `dscan-windows-x64.zip` containing `dscan.exe` at the top level. Scoop extracts zip files directly into the app version folder (`~/scoop/apps/dscan/<version>/`). This flat structure ensures `bin: "dscan.exe"` locates the binary immediately without path prefixes.
- **Shortcuts Array**: `shortcuts: [["dscan-gui.exe", "dscan GUI"]]` instructs Scoop to generate a native Windows Start Menu shortcut during installation and cleanly unregister it during uninstall.
- **Lowercase Hash Requirement**: Scoop requires SHA256 hashes to be in lowercase hexadecimal notation. Using uppercase strings will trigger verification warnings.

---

## 4. Upstream Artifact Optimization (`release.yml`)

To optimize compatibility across all three package managers, the release packaging step in `.github/workflows/release.yml` should be augmented to co-locate license and documentation files inside the distributed archives.

### Recommended Packaging Step Update
In `.github/workflows/release.yml`:
```yaml
      - name: Package Linux artifact
        if: matrix.os == 'ubuntu-latest'
        run: |
          mkdir -p dist-pack
          cp target/${{ matrix.target }}/release/${{ matrix.bin_name }} dist-pack/
          cp LICENSE-MIT LICENSE-APACHE README.md dist-pack/
          tar -czf ${{ matrix.asset_name }} -C dist-pack .
          sha256sum ${{ matrix.asset_name }} > ${{ matrix.asset_name }}.sha256

      - name: Package Windows artifact
        if: matrix.os == 'windows-latest'
        shell: pwsh
        run: |
          mkdir dist-pack-win
          Copy-Item target/${{ matrix.target }}/release/${{ matrix.bin_name }} dist-pack-win/
          Copy-Item LICENSE-MIT, LICENSE-APACHE, README.md dist-pack-win/
          Compress-Archive -Path dist-pack-win/* -DestinationPath ${{ matrix.asset_name }}
          $hash = (Get-FileHash -Path ${{ matrix.asset_name }} -Algorithm SHA256).Hash.ToLower()
          "$hash  ${{ matrix.asset_name }}" | Out-File -FilePath "${{ matrix.asset_name }}.sha256" -Encoding ascii
```

---

## 5. Unified Package Manager Release Matrix

| Package Manager | Target Systems | Channel / Repository | Package Name | Update Trigger |
|:---|:---|:---|:---|:---|
| **AUR (Prebuilt)** | Arch Linux (x86_64, aarch64) | `aur.archlinux.org` | `dscan-bin` | Push to AUR master via CI |
| **AUR (GUI)** | Arch Linux (x86_64) | `aur.archlinux.org` | `dscan-gui-bin` | Push to AUR master via CI |
| **AUR (Source)** | Arch Linux (x86_64, aarch64) | `aur.archlinux.org` | `dscan` | Manual or CI bump |
| **Homebrew** | Linux (x86_64, aarch64) | `AhooraZen/homebrew-dscan` | `dscan` | GitHub Actions API Push |
| **Scoop (CLI)** | Windows (x64) | `AhooraZen/scoop-dscan` + Main | `dscan` | Scoop checkver + Action Dispatch |
| **Scoop (GUI)** | Windows (x64) | `AhooraZen/scoop-dscan` + Extras | `dscan-gui` | Scoop checkver + Action Dispatch |

---

## 6. Implementation Checklist & Execution Order

- [ ] **Phase 1: Upstream Asset Preparation**
  - [ ] Add `LICENSE-MIT` and `README.md` into release `.tar.gz` and `.zip` archives in `.github/workflows/release.yml`.
  - [ ] Ensure `SHA256SUMS.txt` is published as a first-class release asset.
- [ ] **Phase 2: Arch Linux (AUR) Launch**
  - [ ] Create `aur.archlinux.org` account and configure SSH key.
  - [ ] Initialize `dscan-bin` repository on AUR and push validated `PKGBUILD` and `.SRCINFO`.
  - [ ] Initialize `dscan-gui-bin` repository with `dscan-gui.desktop` and push.
  - [ ] Store `AUR_SSH_PRIVATE_KEY` in GitHub repository secrets.
- [ ] **Phase 3: Homebrew Tap Launch**
  - [ ] Create GitHub repository `AhooraZen/homebrew-dscan`.
  - [ ] Commit initial `Formula/dscan.rb` and README.
  - [ ] Verify formula passes `brew audit --strict`.
  - [ ] Store `HOMEBREW_TAP_TOKEN` in GitHub repository secrets.
- [ ] **Phase 4: Scoop Bucket Launch**
  - [ ] Create GitHub repository `AhooraZen/scoop-dscan`.
  - [ ] Add `bucket/dscan.json` and `bucket/dscan-gui.json`.
  - [ ] Configure `ScoopInstaller/GithubActions` auto-updater.
  - [ ] Store `SCOOP_BUCKET_TOKEN` in GitHub repository secrets.
- [ ] **Phase 5: Release Workflow Integration**
  - [ ] Append package manager dispatch jobs to `.github/workflows/release.yml`.
  - [ ] Tag new release (e.g., `v0.6.2`) and observe end-to-end multi-platform rollout.
