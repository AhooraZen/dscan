package com.dscan.app.ui

import java.util.Locale

fun formatBytes(bytes: Long): String {
    val b = if (bytes < 0) 0L else bytes
    return when {
        b >= 1024L * 1024L * 1024L * 1024L -> String.format(Locale.US, "%.2f TiB", b.toDouble() / (1024.0 * 1024.0 * 1024.0 * 1024.0))
        b >= 1024L * 1024L * 1024L -> String.format(Locale.US, "%.2f GiB", b.toDouble() / (1024.0 * 1024.0 * 1024.0))
        b >= 1024L * 1024L -> String.format(Locale.US, "%.1f MiB", b.toDouble() / (1024.0 * 1024.0))
        b >= 1024L -> String.format(Locale.US, "%.1f KiB", b.toDouble() / 1024.0)
        else -> "$b B"
    }
}
