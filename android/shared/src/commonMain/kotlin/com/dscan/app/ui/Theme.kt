package com.dscan.app.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

// Parch Linux Design Tokens - Dark
val CanvasDark = Color(0xFF0C1017)
val SurfaceDark = Color(0xFF151C28)
val SurfaceVariantDark = Color(0xFF1B2433)
val SurfaceHoverDark = Color(0xFF243044)
val BorderDark = Color(0xFF212C3D)
val BrandBlue = Color(0xFF0080FF)
val AccentCyan = Color(0xFF00EAFF)
val AccentGreen = Color(0xFF00FF80)
val AccentAmber = Color(0xFFFBBF24)
val AccentRose = Color(0xFFF43F5E)
val TextPrimaryDark = Color(0xFFF8FAFC)
val TextSecondaryDark = Color(0xFF94A3B8)
val TextMutedDark = Color(0xFF64748B)

// Parch Linux Design Tokens - Light
val CanvasLight = Color(0xFFF8FAFC)
val SurfaceLight = Color(0xFFFFFFFF)
val SurfaceVariantLight = Color(0xFFF1F5F9)
val SurfaceHoverLight = Color(0xFFE2E8F0)
val BorderLight = Color(0xFFE2E8F0)
val TextPrimaryLight = Color(0xFF0F172A)
val TextSecondaryLight = Color(0xFF475569)
val TextMutedLight = Color(0xFF94A3B8)

val ExtensionPalette = listOf(
    Color(0xFF0080FF), // Primary Blue
    Color(0xFF00EAFF), // Cyan
    Color(0xFF00FF80), // Green
    Color(0xFFFBBF24), // Amber
    Color(0xFFF43F5E), // Rose
    Color(0xFFA855F7), // Purple
    Color(0xFFEC4899), // Pink
    Color(0xFF38BDF8), // Sky
    Color(0xFF34D399), // Emerald
    Color(0xFFFB923C), // Orange
    Color(0xFF818CF8), // Indigo
    Color(0xFFE879F9), // Fuchsia
    Color(0xFF2DD4BF), // Teal
    Color(0xFFF87171), // Red
    Color(0xFFA3E635), // Lime
    Color(0xFF64748B)  // Slate
)

fun getExtensionColor(ext: String): Color {
    val lower = ext.trim().lowercase()
    return when (lower) {
        "iso", "img", "vmdk", "qcow2" -> ExtensionPalette[13]
        "zip", "tar", "gz", "7z", "zst", "xz", "bz2", "rar" -> ExtensionPalette[9]
        "apk", "deb", "rpm", "pkg" -> ExtensionPalette[3]
        "sh", "bin", "exe", "AppImage" -> ExtensionPalette[2]
        "rs", "c", "cpp", "h", "hpp", "java", "kt", "go" -> ExtensionPalette[0]
        "ts", "js", "py", "json", "toml", "yaml", "yml" -> ExtensionPalette[1]
        "mp4", "mkv", "mov", "webm", "avi" -> ExtensionPalette[7]
        "mp3", "flac", "wav", "ogg", "aac" -> ExtensionPalette[8]
        "png", "jpg", "jpeg", "webp", "svg", "gif" -> ExtensionPalette[6]
        "pdf", "doc", "docx", "txt", "md", "csv" -> ExtensionPalette[10]
        "db", "sqlite", "sql", "parquet" -> ExtensionPalette[5]
        "so", "dll", "dylib", "dex" -> ExtensionPalette[11]
        "log", "tmp", "cache", "bak" -> ExtensionPalette[15]
        else -> {
            if (lower.isEmpty()) ExtensionPalette[15]
            else {
                val hash = lower.fold(0) { acc, c -> (acc * 31 + c.code) and 0x7FFFFFFF }
                ExtensionPalette[hash % 15]
            }
        }
    }
}

val ParchDarkColorScheme = darkColorScheme(
    primary = BrandBlue,
    secondary = AccentCyan,
    tertiary = AccentGreen,
    background = CanvasDark,
    surface = SurfaceDark,
    surfaceVariant = SurfaceVariantDark,
    onPrimary = Color.White,
    onSecondary = Color.Black,
    onTertiary = Color.Black,
    onBackground = TextPrimaryDark,
    onSurface = TextPrimaryDark,
    onSurfaceVariant = TextSecondaryDark,
    outline = BorderDark,
    error = AccentRose,
    onError = Color.White
)

val ParchLightColorScheme = lightColorScheme(
    primary = BrandBlue,
    secondary = Color(0xFF00A2B3),
    tertiary = Color(0xFF00A854),
    background = CanvasLight,
    surface = SurfaceLight,
    surfaceVariant = SurfaceVariantLight,
    onPrimary = Color.White,
    onSecondary = Color.White,
    onTertiary = Color.White,
    onBackground = TextPrimaryLight,
    onSurface = TextPrimaryLight,
    onSurfaceVariant = TextSecondaryLight,
    outline = BorderLight,
    error = Color(0xFFDC2626),
    onError = Color.White
)

@Composable
fun DscanTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit
) {
    val colorScheme = if (darkTheme) ParchDarkColorScheme else ParchLightColorScheme
    MaterialTheme(
        colorScheme = colorScheme,
        content = content
    )
}
