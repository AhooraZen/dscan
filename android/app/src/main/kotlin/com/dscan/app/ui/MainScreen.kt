package com.dscan.app.ui

import android.view.HapticFeedbackConstants
import androidx.compose.animation.*
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import com.dscan.app.ExtensionStat
import com.dscan.app.ScanProgress
import com.dscan.app.TreemapNode

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MainScreen(
    currentPath: String,
    onPathChange: (String) -> Unit,
    progress: ScanProgress,
    isScanning: Boolean,
    treemapNodes: List<TreemapNode>,
    extensionStats: List<ExtensionStat>,
    onStartScan: () -> Unit,
    onCancelScan: () -> Unit,
    onBrowseFolder: () -> Unit,
    onNavigateSettings: () -> Unit,
    modifier: Modifier = Modifier
) {
    var selectedTab by rememberSaveable { mutableIntStateOf(0) }
    var selectedNode by remember { mutableStateOf<TreemapNode?>(null) }
    val view = LocalView.current

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            "dscan",
                            style = MaterialTheme.typography.titleLarge,
                            color = MaterialTheme.colorScheme.primary
                        )
                        Spacer(modifier = Modifier.width(8.dp))
                        Text(
                            "Disk Space Analyzer",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                },
                actions = {
                    IconButton(onClick = onNavigateSettings) {
                        Icon(Icons.Default.Settings, contentDescription = "Settings")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surface
                )
            )
        },
        bottomBar = {
            NavigationBar {
                val items = listOf(
                    Triple("Treemap", Icons.Default.GridView, 0),
                    Triple("Directory", Icons.Default.FolderOpen, 1),
                    Triple("File Types", Icons.Default.PieChart, 2)
                )
                items.forEach { (label, icon, index) ->
                    NavigationBarItem(
                        selected = selectedTab == index,
                        onClick = { selectedTab = index },
                        icon = { Icon(icon, contentDescription = label) },
                        label = { Text(label) }
                    )
                }
            }
        },
        floatingActionButton = {
            if (isScanning) {
                ExtendedFloatingActionButton(
                    onClick = onCancelScan,
                    containerColor = MaterialTheme.colorScheme.error,
                    contentColor = MaterialTheme.colorScheme.onError
                ) {
                    Icon(Icons.Default.Close, contentDescription = null)
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("Cancel")
                }
            } else {
                ExtendedFloatingActionButton(
                    onClick = onStartScan,
                    containerColor = MaterialTheme.colorScheme.primary,
                    contentColor = MaterialTheme.colorScheme.onPrimary
                ) {
                    Icon(Icons.Default.PlayArrow, contentDescription = null)
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("Scan")
                }
            }
        }
    ) { innerPadding ->
        Column(
            modifier = modifier
                .fillMaxSize()
                .padding(innerPadding)
                .background(MaterialTheme.colorScheme.background)
        ) {
            // Storage quick selector + Browse button
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                StoragePicker(
                    selectedPath = currentPath,
                    onPathSelected = onPathChange,
                    modifier = Modifier.weight(1f)
                )
                Spacer(modifier = Modifier.width(8.dp))
                FilledTonalButton(onClick = onBrowseFolder) {
                    Icon(Icons.Default.FolderOpen, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(modifier = Modifier.width(4.dp))
                    Text("Browse")
                }
            }

            // Scanning gauge or progress stats
            AnimatedContent(
                targetState = isScanning,
                transitionSpec = {
                    fadeIn() + slideInVertically() togetherWith fadeOut() + slideOutVertically()
                },
                label = "scan_state"
            ) { scanning ->
                if (scanning) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 8.dp),
                        contentAlignment = Alignment.Center
                    ) {
                        ScanGauge(progress = progress, modifier = Modifier.padding(8.dp))
                    }
                } else if (progress.totalFiles > 0) {
                    // Compact stats after scan
                    Card(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp, vertical = 8.dp),
                        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
                    ) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(12.dp),
                            horizontalArrangement = Arrangement.SpaceBetween
                        ) {
                            Text(
                                "Total: ${formatBytes(progress.totalBytes)}",
                                style = MaterialTheme.typography.titleMedium,
                                color = MaterialTheme.colorScheme.primary
                            )
                            Text(
                                "${progress.totalFiles} files  •  ${progress.elapsedMillis}ms",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    }
                }
            }

            // Content Area
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
            ) {
                AnimatedContent(
                    targetState = selectedTab,
                    transitionSpec = {
                        fadeIn() + slideInHorizontally { if (targetState > initialState) it / 4 else -it / 4 } togetherWith
                                fadeOut() + slideOutHorizontally { if (targetState > initialState) -it / 4 else it / 4 }
                    },
                    label = "tab"
                ) { tab ->
                    when (tab) {
                        0 -> {
                            if (treemapNodes.isEmpty()) {
                                Box(
                                    modifier = Modifier.fillMaxSize(),
                                    contentAlignment = Alignment.Center
                                ) {
                                    Text(
                                        if (isScanning) "Building treemap..." else "Tap Scan to visualize disk space",
                                        color = MaterialTheme.colorScheme.onSurfaceVariant
                                    )
                                }
                            } else {
                                Column(modifier = Modifier.fillMaxSize()) {
                                    if (selectedNode != null) {
                                        Surface(
                                            color = MaterialTheme.colorScheme.surfaceVariant,
                                            modifier = Modifier.fillMaxWidth()
                                        ) {
                                            Row(
                                                modifier = Modifier.padding(horizontal = 16.dp, vertical = 6.dp),
                                                horizontalArrangement = Arrangement.SpaceBetween,
                                                verticalAlignment = Alignment.CenterVertically
                                            ) {
                                                Text(
                                                    "${selectedNode?.name} (${formatBytes(selectedNode?.totalBytes ?: 0)})",
                                                    style = MaterialTheme.typography.bodyMedium
                                                )
                                                IconButton(onClick = { selectedNode = null }) {
                                                    Icon(Icons.Default.Close, contentDescription = "Deselect")
                                                }
                                            }
                                        }
                                    }
                                    TreemapCanvas(
                                        nodes = treemapNodes,
                                        selectedNode = selectedNode,
                                        onNodeSelected = {
                                            view.performHapticFeedback(HapticFeedbackConstants.CLOCK_TICK)
                                            selectedNode = it
                                        },
                                        modifier = Modifier.weight(1f)
                                    )
                                    // Color legend
                                    ColorLegend(
                                        extensionStats = extensionStats,
                                        modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp)
                                    )
                                }
                            }
                        }
                        1 -> {
                            DirectoryList(
                                nodes = treemapNodes,
                                selectedNode = selectedNode,
                                onNodeClick = {
                                    view.performHapticFeedback(HapticFeedbackConstants.CLOCK_TICK)
                                    selectedNode = it
                                }
                            )
                        }
                        2 -> {
                            ExtensionBreakdownList(stats = extensionStats)
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun ColorLegend(
    extensionStats: List<ExtensionStat>,
    modifier: Modifier = Modifier
) {
    if (extensionStats.isEmpty()) return
    Row(
        modifier = modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        extensionStats.take(12).forEach { stat ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(
                    modifier = Modifier
                        .size(10.dp)
                        .clip(RoundedCornerShape(2.dp))
                        .background(getExtensionColor(stat.extension))
                )
                Spacer(modifier = Modifier.width(3.dp))
                Text(
                    text = stat.extension.ifEmpty { "?" },
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}
