package com.dscan.app.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

val BgDark = Color(0xFF0F172A)
val SurfaceDark = Color(0xFF1E293B)
val SurfaceHover = Color(0xFF334155)
val BorderDark = Color(0xFF334155)
val TextPrimaryDark = Color(0xFFF8FAFC)
val TextMutedDark = Color(0xFF94A3B8)
val AccentSky = Color(0xFF38BDF8)
val AccentEmerald = Color(0xFF34D399)
val AccentAmber = Color(0xFFFBBF24)
val AccentRose = Color(0xFFF43F5E)

val BgLight = Color(0xFFF8FAFC)
val SurfaceLight = Color(0xFFFFFFFF)
val BorderLight = Color(0xFFE2E8F0)
val TextPrimaryLight = Color(0xFF0F172A)
val TextMutedLight = Color(0xFF64748B)

val ExtensionPalette = listOf(
    Color(0xFFF87171), // Red
    Color(0xFFFB923C), // Orange
    Color(0xFFFBBF24), // Amber
    Color(0xFFA3E635), // Lime
    Color(0xFF34D399), // Emerald
    Color(0xFF2DD4BF), // Teal
    Color(0xFF38BDF8), // Sky
    Color(0xFF60A5FA), // Blue
    Color(0xFF818CF8), // Indigo
    Color(0xFFA78BFA), // Violet
    Color(0xFFC084FC), // Purple
    Color(0xFFE879F9), // Fuchsia
    Color(0xFFF472B6), // Pink
    Color(0xFFFB7185), // Rose
    Color(0xFF22D3EE), // Cyan
    Color(0xFF94A3B8)  // Slate
)

fun getExtensionColor(ext: String): Color {
    val lower = ext.trim().lowercase()
    return when (lower) {
        ".iso", ".img", ".vmdk" -> ExtensionPalette[0]
        ".zip", ".tar", ".gz", ".7z", ".zst" -> ExtensionPalette[1]
        ".apk", ".deb", ".rpm" -> ExtensionPalette[2]
        ".sh", ".bin", ".exe" -> ExtensionPalette[3]
        ".rs", ".c", ".cpp", ".java", ".kt" -> ExtensionPalette[4]
        ".ts", ".js", ".py", ".json" -> ExtensionPalette[5]
        ".mp4", ".mkv", ".mov", ".webm" -> ExtensionPalette[6]
        ".mp3", ".flac", ".wav", ".ogg" -> ExtensionPalette[7]
        ".png", ".jpg", ".jpeg", ".webp", ".svg" -> ExtensionPalette[8]
        ".pdf", ".doc", ".docx", ".txt", ".md" -> ExtensionPalette[9]
        ".db", ".sqlite", ".sql" -> ExtensionPalette[10]
        ".so", ".dex" -> ExtensionPalette[12]
        ".log", ".tmp" -> ExtensionPalette[13]
        else -> {
            if (lower.isEmpty()) ExtensionPalette[15]
            else {
                val hash = lower.fold(0) { acc, c -> (acc * 31 + c.code) and 0x7FFFFFFF }
                ExtensionPalette[hash % 15]
            }
        }
    }
}

private val DarkColorScheme = darkColorScheme(
    primary = AccentSky,
    secondary = AccentEmerald,
    tertiary = AccentAmber,
    background = BgDark,
    surface = SurfaceDark,
    onPrimary = Color.Black,
    onSecondary = Color.Black,
    onBackground = TextPrimaryDark,
    onSurface = TextPrimaryDark,
    surfaceVariant = SurfaceHover,
    onSurfaceVariant = TextMutedDark,
    outline = BorderDark
)

private val LightColorScheme = lightColorScheme(
    primary = Color(0xFF0284C7),
    secondary = Color(0xFF059669),
    tertiary = Color(0xFFD97706),
    background = BgLight,
    surface = SurfaceLight,
    onPrimary = Color.White,
    onSecondary = Color.White,
    onBackground = TextPrimaryLight,
    onSurface = TextPrimaryLight,
    surfaceVariant = Color(0xFFF1F5F9),
    onSurfaceVariant = TextMutedLight,
    outline = BorderLight
)

@Composable
fun DscanTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit
) {
    val colorScheme = if (darkTheme) DarkColorScheme else LightColorScheme
    MaterialTheme(
        colorScheme = colorScheme,
        content = content
    )
}
