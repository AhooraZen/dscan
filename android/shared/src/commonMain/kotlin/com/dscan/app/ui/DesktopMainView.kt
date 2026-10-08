package com.dscan.app.ui

import androidx.compose.animation.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.dscan.app.DriveInfo
import com.dscan.app.DscanBridge
import com.dscan.app.ExtensionStat
import com.dscan.app.ScanProgress
import com.dscan.app.TreemapNode

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DesktopMainView(
    targetPath: String,
    onPathChange: (String) -> Unit,
    progress: ScanProgress,
    isScanning: Boolean,
    treemapNodes: List<TreemapNode>,
    extensionStats: List<ExtensionStat>,
    drives: List<DriveInfo>,
    isDarkTheme: Boolean,
    onToggleTheme: () -> Unit,
    onStartScan: () -> Unit,
    onCancelScan: () -> Unit,
    onBrowseFolder: () -> Unit,
    onOpenSettings: () -> Unit,
    searchQuery: String,
    onSearchQueryChange: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    var selectedSidebarTab by rememberSaveable { mutableIntStateOf(0) }
    var selectedNode by remember { mutableStateOf<TreemapNode?>(null) }
    var sidebarWidth by remember { mutableStateOf(360.dp) }

    val totalBytes = remember(progress, treemapNodes) {
        if (progress.totalBytes > 0) progress.totalBytes
        else treemapNodes.firstOrNull()?.totalBytes ?: 0L
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
    ) {
        // 1. Top Header Bar
        Surface(
            color = MaterialTheme.colorScheme.surface,
            border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.5f)),
            modifier = Modifier.fillMaxWidth()
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                // Brand Logo & Badge
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.padding(end = 16.dp)
                ) {
                    Text(
                        "⚡ dscan",
                        style = MaterialTheme.typography.titleLarge.copy(
                            fontWeight = FontWeight.Black,
                            letterSpacing = (-0.5).sp
                        ),
                        color = MaterialTheme.colorScheme.primary
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Surface(
                        shape = RoundedCornerShape(4.dp),
                        color = MaterialTheme.colorScheme.surfaceVariant
                    ) {
                        Text(
                            "v0.7.0",
                            style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.Bold),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp)
                        )
                    }
                }

                // Drive quick picks (if any detected)
                if (drives.isNotEmpty()) {
                    Row(
                        modifier = Modifier.padding(end = 12.dp),
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        drives.take(3).forEach { drive ->
                            val isSelected = targetPath == drive.mountPoint
                            FilterChip(
                                selected = isSelected,
                                onClick = { onPathChange(drive.mountPoint) },
                                label = {
                                    Text(
                                        drive.name.ifEmpty { drive.mountPoint },
                                        style = MaterialTheme.typography.labelSmall
                                    )
                                },
                                leadingIcon = {
                                    Icon(Icons.Default.Storage, contentDescription = null, modifier = Modifier.size(14.dp))
                                }
                            )
                        }
                    }
                }

                // Path Input Field
                OutlinedTextField(
                    value = targetPath,
                    onValueChange = onPathChange,
                    modifier = Modifier
                        .weight(1f)
                        .height(50.dp),
                    singleLine = true,
                    textStyle = MaterialTheme.typography.bodyMedium,
                    placeholder = { Text("Enter directory path to scan...", style = MaterialTheme.typography.bodyMedium) },
                    leadingIcon = {
                        Icon(Icons.Default.Folder, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                    },
                    shape = RoundedCornerShape(8.dp),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedBorderColor = MaterialTheme.colorScheme.primary,
                        unfocusedBorderColor = MaterialTheme.colorScheme.outline
                    )
                )

                Spacer(modifier = Modifier.width(8.dp))

                // Browse Button
                OutlinedButton(
                    onClick = onBrowseFolder,
                    shape = RoundedCornerShape(8.dp),
                    modifier = Modifier.height(48.dp)
                ) {
                    Icon(Icons.Default.FolderOpen, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(modifier = Modifier.width(6.dp))
                    Text("Browse")
                }

                Spacer(modifier = Modifier.width(8.dp))

                // Scan / Cancel Action Button
                if (isScanning) {
                    Button(
                        onClick = onCancelScan,
                        shape = RoundedCornerShape(8.dp),
                        colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error),
                        modifier = Modifier.height(48.dp)
                    ) {
                        Icon(Icons.Default.Close, contentDescription = null, modifier = Modifier.size(18.dp))
                        Spacer(modifier = Modifier.width(6.dp))
                        Text("Cancel")
                    }
                } else {
                    Button(
                        onClick = onStartScan,
                        shape = RoundedCornerShape(8.dp),
                        colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary),
                        modifier = Modifier.height(48.dp)
                    ) {
                        Icon(Icons.Default.PlayArrow, contentDescription = null, modifier = Modifier.size(18.dp))
                        Spacer(modifier = Modifier.width(6.dp))
                        Text("Scan")
                    }
                }

                Spacer(modifier = Modifier.width(12.dp))

                // Search Box
                OutlinedTextField(
                    value = searchQuery,
                    onValueChange = onSearchQueryChange,
                    modifier = Modifier
                        .width(200.dp)
                        .height(50.dp),
                    singleLine = true,
                    placeholder = { Text("Filter (Ctrl+F)", style = MaterialTheme.typography.bodySmall) },
                    leadingIcon = {
                        Icon(Icons.Default.Search, contentDescription = null, modifier = Modifier.size(16.dp))
                    },
                    trailingIcon = {
                        if (searchQuery.isNotEmpty()) {
                            IconButton(onClick = { onSearchQueryChange("") }) {
                                Icon(Icons.Default.Clear, contentDescription = "Clear", modifier = Modifier.size(16.dp))
                            }
                        }
                    },
                    shape = RoundedCornerShape(8.dp),
                    textStyle = MaterialTheme.typography.bodySmall
                )

                Spacer(modifier = Modifier.width(8.dp))

                // Theme Toggle Button
                IconButton(onClick = onToggleTheme) {
                    Icon(
                        if (isDarkTheme) Icons.Default.LightMode else Icons.Default.DarkMode,
                        contentDescription = "Toggle Theme"
                    )
                }

                // Settings Button
                IconButton(onClick = onOpenSettings) {
                    Icon(Icons.Default.Settings, contentDescription = "Settings")
                }
            }
        }

        // Scanning Active Banner
        if (isScanning) {
            LinearProgressIndicator(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(3.dp),
                color = AccentCyan,
                trackColor = MaterialTheme.colorScheme.surfaceVariant
            )
        }

        // 2. Main Workspace Split View
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
        ) {
            // Left Sidebar Panel
            Surface(
                modifier = Modifier
                    .width(sidebarWidth)
                    .fillMaxHeight(),
                color = MaterialTheme.colorScheme.surface,
                border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.5f))
            ) {
                Column(modifier = Modifier.fillMaxSize()) {
                    // Sidebar Navigation Tabs
                    PrimaryTabRow(
                        selectedTabIndex = selectedSidebarTab,
                        containerColor = MaterialTheme.colorScheme.surface,
                        contentColor = MaterialTheme.colorScheme.primary
                    ) {
                        Tab(
                            selected = selectedSidebarTab == 0,
                            onClick = { selectedSidebarTab = 0 },
                            text = { Text("Tree", style = MaterialTheme.typography.labelMedium) },
                            icon = { Icon(Icons.Default.AccountTree, contentDescription = null, modifier = Modifier.size(16.dp)) }
                        )
                        Tab(
                            selected = selectedSidebarTab == 1,
                            onClick = { selectedSidebarTab = 1 },
                            text = { Text("Top Files", style = MaterialTheme.typography.labelMedium) },
                            icon = { Icon(Icons.Default.Leaderboard, contentDescription = null, modifier = Modifier.size(16.dp)) }
                        )
                        Tab(
                            selected = selectedSidebarTab == 2,
                            onClick = { selectedSidebarTab = 2 },
                            text = { Text("Types", style = MaterialTheme.typography.labelMedium) },
                            icon = { Icon(Icons.Default.PieChart, contentDescription = null, modifier = Modifier.size(16.dp)) }
                        )
                    }

                    // Tab Content
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .weight(1f)
                    ) {
                        when (selectedSidebarTab) {
                            0 -> DirectoryExplorer(
                                nodes = treemapNodes,
                                onNodeSelected = { selectedNode = it }
                            )
                            1 -> StorageHogsList(
                                nodes = treemapNodes,
                                onNodeSelected = { selectedNode = it }
                            )
                            2 -> ExtensionBreakdownList(
                                stats = extensionStats
                            )
                        }
                    }
                }
            }

            // Right Main Treemap Canvas Area
            Box(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxHeight()
                    .background(MaterialTheme.colorScheme.background)
            ) {
                TreemapView(
                    nodes = treemapNodes,
                    targetPath = targetPath,
                    searchQuery = searchQuery,
                    onNodeSelected = { selectedNode = it }
                )
            }
        }

        // 3. Bottom Status Bar
        Surface(
            color = MaterialTheme.colorScheme.surface,
            border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.5f)),
            modifier = Modifier.fillMaxWidth()
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 6.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                // Left: Selected Node / Active Path
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.weight(1f)
                ) {
                    val statusText = when {
                        isScanning && progress.currentPath.isNotEmpty() -> "Scanning: ${progress.currentPath}"
                        selectedNode != null -> "Selected: ${selectedNode?.name} (${formatBytes(selectedNode?.totalBytes ?: 0L)})"
                        progress.isComplete -> "Scan complete: ${progress.totalFiles} files indexed"
                        else -> "Ready"
                    }

                    Text(
                        statusText,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }

                Spacer(modifier = Modifier.width(16.dp))

                // Right: Metrics Summary
                Row(
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        "Total: ${formatBytes(totalBytes)}",
                        style = MaterialTheme.typography.bodySmall.copy(fontWeight = FontWeight.Bold),
                        color = MaterialTheme.colorScheme.primary
                    )
                    Text(
                        "Files: ${progress.totalFiles}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    if (progress.filesPerSec > 0.0) {
                        Text(
                            "Speed: ${progress.filesPerSec.toInt()} files/s (${formatThroughput(progress.bytesPerSec)})",
                            style = MaterialTheme.typography.bodySmall,
                            color = AccentCyan
                        )
                    }
                    if (progress.activeWorkers > 0) {
                        Text(
                            "Workers: ${progress.activeWorkers}",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                    Text(
                        "Duration: ${formatDuration(progress.elapsedMillis)}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }
    }
}
