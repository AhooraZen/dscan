use gpui::Rgba;

pub const fn const_rgb(hex: u32) -> Rgba {
    let [_, r, g, b] = hex.to_be_bytes();
    Rgba {
        r: (r as f32) / 255.0,
        g: (g as f32) / 255.0,
        b: (b as f32) / 255.0,
        a: 1.0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

impl ThemeMode {
    pub const fn toggle(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }

    pub const fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    pub const fn colors(self) -> ThemeColors {
        match self {
            Self::Dark => ThemeColors::dark(),
            Self::Light => ThemeColors::light(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColors {
    pub bg: Rgba,
    pub surface: Rgba,
    pub surface_hover: Rgba,
    pub surface_active: Rgba,
    pub border: Rgba,
    pub border_light: Rgba,
    pub text_primary: Rgba,
    pub text_muted: Rgba,
    pub text_dim: Rgba,
    pub accent_blue: Rgba,
    pub accent_green: Rgba,
    pub accent_amber: Rgba,
    pub accent_red: Rgba,
}

impl ThemeColors {
    pub const fn dark() -> Self {
        Self {
            bg: const_rgb(0x0F172A),             // Slate 900
            surface: const_rgb(0x1E293B),        // Slate 800
            surface_hover: const_rgb(0x334155),  // Slate 700
            surface_active: const_rgb(0x475569), // Slate 600
            border: const_rgb(0x334155),         // Slate 700
            border_light: const_rgb(0x475569),   // Slate 600
            text_primary: const_rgb(0xF8FAFC),   // Slate 50
            text_muted: const_rgb(0x94A3B8),     // Slate 400
            text_dim: const_rgb(0x64748B),       // Slate 500
            accent_blue: const_rgb(0x38BDF8),    // Sky 400
            accent_green: const_rgb(0x34D399),   // Emerald 400
            accent_amber: const_rgb(0xFBBF24),   // Amber 400
            accent_red: const_rgb(0xEF4444),     // Red 500
        }
    }

    pub const fn light() -> Self {
        Self {
            bg: const_rgb(0xF8FAFC),             // Slate 50
            surface: const_rgb(0xFFFFFF),        // White
            surface_hover: const_rgb(0xF1F5F9),  // Slate 100
            surface_active: const_rgb(0xE2E8F0), // Slate 200
            border: const_rgb(0xE2E8F0),         // Slate 200
            border_light: const_rgb(0xCBD5E1),   // Slate 300
            text_primary: const_rgb(0x0F172A),   // Slate 900
            text_muted: const_rgb(0x475569),     // Slate 600
            text_dim: const_rgb(0x64748B),       // Slate 500
            accent_blue: const_rgb(0x0284C7),    // Sky 600
            accent_green: const_rgb(0x059669),   // Emerald 600
            accent_amber: const_rgb(0xD97706),   // Amber 600
            accent_red: const_rgb(0xDC2626),     // Red 600
        }
    }
}

// Default constants for backward compatibility
pub const BG_DARK: Rgba = const_rgb(0x0F172A);
pub const SURFACE_DARK: Rgba = const_rgb(0x1E293B);
pub const SURFACE_HOVER: Rgba = const_rgb(0x334155);
pub const BORDER_DARK: Rgba = const_rgb(0x334155);
pub const BORDER_LIGHT: Rgba = const_rgb(0x475569);
pub const TEXT_PRIMARY: Rgba = const_rgb(0xF8FAFC);
pub const TEXT_MUTED: Rgba = const_rgb(0x94A3B8);
pub const TEXT_DIM: Rgba = const_rgb(0x64748B);
pub const ACCENT_BLUE: Rgba = const_rgb(0x38BDF8);
pub const ACCENT_GREEN: Rgba = const_rgb(0x34D399);
pub const ACCENT_AMBER: Rgba = const_rgb(0xFBBF24);
pub const ACCENT_RED: Rgba = const_rgb(0xEF4444);

// 16 distinct perceptual OKLCH-derived colors for file extensions
pub const EXTENSION_PALETTE: [Rgba; 16] = [
    const_rgb(0xF87171), // 0: Red (images/ISOs)
    const_rgb(0xFB923C), // 1: Orange (archives: .zip, .tar, .gz)
    const_rgb(0xFBBF24), // 2: Amber (packages: .deb, .rpm, .pkg)
    const_rgb(0xA3E635), // 3: Lime (executables/scripts: .sh, .bin)
    const_rgb(0x34D399), // 4: Emerald (source code: .rs, .c, .cpp)
    const_rgb(0x2DD4BF), // 5: Teal (web code: .ts, .js, .json)
    const_rgb(0x38BDF8), // 6: Sky (video: .mp4, .mkv, .mov)
    const_rgb(0x60A5FA), // 7: Blue (audio: .mp3, .flac, .wav)
    const_rgb(0x818CF8), // 8: Indigo (graphics: .png, .jpg, .svg)
    const_rgb(0xA78BFA), // 9: Violet (documents: .pdf, .docx, .txt)
    const_rgb(0xC084FC), // 10: Purple (databases: .db, .sqlite)
    const_rgb(0xE879F9), // 11: Fuchsia (3D/CAD: .blend, .obj)
    const_rgb(0xF472B6), // 12: Pink (binaries: .so, .dll, .wasm)
    const_rgb(0xFB7185), // 13: Rose (logs/temp: .log, .tmp)
    const_rgb(0x22D3EE), // 14: Cyan (markup/styles: .html, .css)
    const_rgb(0x94A3B8), // 15: Slate (other/misc/unknown)
];

/// Get color for file extension from 16-color perceptual palette
pub fn extension_color(ext: &str) -> Rgba {
    let normalized = ext.trim().to_ascii_lowercase();
    match normalized.as_str() {
        ".iso" | ".img" | ".vmdk" | ".qcow2" => EXTENSION_PALETTE[0],
        ".zip" | ".tar" | ".gz" | ".7z" | ".xz" | ".zst" | ".bz2" => EXTENSION_PALETTE[1],
        ".deb" | ".rpm" | ".pkg" | ".apk" => EXTENSION_PALETTE[2],
        ".sh" | ".bash" | ".exe" | ".bin" => EXTENSION_PALETTE[3],
        ".rs" | ".c" | ".cpp" | ".h" | ".hpp" | ".go" | ".java" => EXTENSION_PALETTE[4],
        ".ts" | ".js" | ".py" | ".json" | ".toml" | ".yaml" => EXTENSION_PALETTE[5],
        ".mp4" | ".mkv" | ".avi" | ".mov" | ".webm" => EXTENSION_PALETTE[6],
        ".mp3" | ".flac" | ".wav" | ".ogg" | ".m4a" => EXTENSION_PALETTE[7],
        ".png" | ".jpg" | ".jpeg" | ".webp" | ".svg" | ".gif" => EXTENSION_PALETTE[8],
        ".pdf" | ".doc" | ".docx" | ".txt" | ".md" => EXTENSION_PALETTE[9],
        ".sqlite" | ".db" | ".sql" => EXTENSION_PALETTE[10],
        ".blend" | ".obj" | ".fbx" | ".gltf" => EXTENSION_PALETTE[11],
        ".so" | ".dll" | ".dylib" | ".wasm" => EXTENSION_PALETTE[12],
        ".log" | ".tmp" | ".bak" => EXTENSION_PALETTE[13],
        ".html" | ".css" | ".scss" | ".xml" => EXTENSION_PALETTE[14],
        "[other]" | "[misc]" | "" => EXTENSION_PALETTE[15],
        _ => {
            let hash = normalized
                .bytes()
                .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
            EXTENSION_PALETTE[(hash as usize) % 15]
        }
    }
}

/// Get extension color by index in sorted table
pub fn extension_color_by_index(index: usize) -> Rgba {
    EXTENSION_PALETTE[index % EXTENSION_PALETTE.len()]
}
