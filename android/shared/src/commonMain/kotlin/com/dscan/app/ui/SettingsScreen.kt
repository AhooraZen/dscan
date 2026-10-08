package com.dscan.app.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt

enum class ThemeMode { System, Light, Dark }

data class ScanSettings(
    val threadCount: Int = 0, // 0 = auto
    val maxTreemapDepth: Int = 6,
    val maxTreemapNodes: Int = 2000,
    val themeMode: ThemeMode = ThemeMode.System
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    settings: ScanSettings,
    onSettingsChange: (ScanSettings) -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surface
                )
            )
        }
    ) { innerPadding ->
        Column(
            modifier = modifier
                .fillMaxSize()
                .padding(innerPadding)
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(24.dp)
        ) {
            // Thread count
            SectionHeader("Scan Threads")
            val threadOptions = listOf(0, 2, 4, 8, 16)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                threadOptions.forEach { count ->
                    val label = if (count == 0) "Auto" else "$count"
                    FilterChip(
                        selected = settings.threadCount == count,
                        onClick = { onSettingsChange(settings.copy(threadCount = count)) },
                        label = { Text(label) }
                    )
                }
            }

            // Max treemap depth
            SectionHeader("Max Treemap Depth: ${settings.maxTreemapDepth}")
            Slider(
                value = settings.maxTreemapDepth.toFloat(),
                onValueChange = {
                    onSettingsChange(settings.copy(maxTreemapDepth = it.roundToInt()))
                },
                valueRange = 2f..8f,
                steps = 5
            )

            // Max treemap nodes
            SectionHeader("Max Treemap Nodes: ${settings.maxTreemapNodes}")
            Slider(
                value = settings.maxTreemapNodes.toFloat(),
                onValueChange = {
                    onSettingsChange(settings.copy(maxTreemapNodes = (it.roundToInt() / 100) * 100))
                },
                valueRange = 500f..5000f,
                steps = 44
            )

            // Theme
            SectionHeader("Theme")
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ThemeMode.entries.forEach { mode ->
                    FilterChip(
                        selected = settings.themeMode == mode,
                        onClick = { onSettingsChange(settings.copy(themeMode = mode)) },
                        label = { Text(mode.name) }
                    )
                }
            }

            // About
            HorizontalDivider()
            SectionHeader("About")
            Text(
                "dscan v0.7.0",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Text(
                "Zero-dependency, multi-threaded disk space analyzer powered by direct Linux/Windows kernel syscalls and Compose Multiplatform.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

@Composable
private fun SectionHeader(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.titleSmall,
        fontWeight = FontWeight.SemiBold,
        color = MaterialTheme.colorScheme.primary
    )
}
