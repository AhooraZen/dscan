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

fun formatThroughput(bytesPerSec: Double): String {
    val b = if (bytesPerSec < 0) 0.0 else bytesPerSec
    return when {
        b >= 1024.0 * 1024.0 * 1024.0 -> String.format(Locale.US, "%.2f GiB/s", b / (1024.0 * 1024.0 * 1024.0))
        b >= 1024.0 * 1024.0 -> String.format(Locale.US, "%.1f MiB/s", b / (1024.0 * 1024.0))
        b >= 1024.0 -> String.format(Locale.US, "%.1f KiB/s", b / 1024.0)
        else -> String.format(Locale.US, "%.0f B/s", b)
    }
}

fun formatDuration(elapsedMillis: Long): String {
    val ms = if (elapsedMillis < 0) 0L else elapsedMillis
    val totalSeconds = ms / 1000
    val minutes = totalSeconds / 60
    val seconds = totalSeconds % 60
    val remMs = ms % 1000
    return if (minutes > 0) {
        String.format(Locale.US, "%dm %02d.%01ds", minutes, seconds, remMs / 100)
    } else {
        String.format(Locale.US, "%d.%02ds", seconds, remMs / 10)
    }
}
