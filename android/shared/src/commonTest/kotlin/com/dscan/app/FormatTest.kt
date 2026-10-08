package com.dscan.app

import com.dscan.app.ui.formatBytes
import com.dscan.app.ui.formatDuration
import com.dscan.app.ui.formatThroughput
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class FormatTest {

    @Test
    fun testFormatBytesZeroAndNegative() {
        assertEquals("0 B", formatBytes(0L))
        assertEquals("0 B", formatBytes(-50L))
    }

    @Test
    fun testFormatBytesUnits() {
        assertEquals("500 B", formatBytes(500L))
        assertEquals("1.0 KiB", formatBytes(1024L))
        assertEquals("1.5 KiB", formatBytes(1536L))
        assertEquals("1.0 MiB", formatBytes(1024L * 1024L))
        assertEquals("1.00 GiB", formatBytes(1024L * 1024L * 1024L))
        assertEquals("2.50 TiB", formatBytes((2.5 * 1024.0 * 1024.0 * 1024.0 * 1024.0).toLong()))
    }

    @Test
    fun testFormatThroughput() {
        assertTrue(formatThroughput(0.0).contains("B/s"))
        assertTrue(formatThroughput(2048.0).contains("KiB/s"))
        assertTrue(formatThroughput(5.0 * 1024.0 * 1024.0).contains("MiB/s"))
    }

    @Test
    fun testFormatDuration() {
        assertTrue(formatDuration(500L).contains("s"))
        assertTrue(formatDuration(65000L).contains("1m"))
    }
}
