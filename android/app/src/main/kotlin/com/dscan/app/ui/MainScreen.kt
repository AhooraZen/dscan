package com.dscan.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
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
    darkTheme: Boolean,
    onToggleDarkTheme: () -> Unit,
    modifier: Modifier = Modifier
) {
    var selectedTab by remember { mutableIntStateOf(0) }
    var selectedNode by remember { mutableStateOf<TreemapNode?>(null) }

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
                    IconButton(onClick = onToggleDarkTheme) {
                        Icon(
                            imageVector = if (darkTheme) Icons.Default.LightMode else Icons.Default.DarkMode,
                            contentDescription = "Toggle Theme"
                        )
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
                .background(MaterialTheme.colorScheme.background)
        ) {
            // Storage quick selector
            StoragePicker(
                selectedPath = currentPath,
                onPathSelected = onPathChange,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
            )

            // Path & Scan Bar
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                OutlinedTextField(
                    value = currentPath,
                    onValueChange = onPathChange,
                    modifier = Modifier.weight(1f),
                    label = { Text("Scan Target") },
                    singleLine = true,
                    enabled = !isScanning
                )
                Spacer(modifier = Modifier.width(8.dp))
                if (isScanning) {
                    Button(
                        onClick = onCancelScan,
                        colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error)
                    ) {
                        Text("Cancel")
                    }
                } else {
                    Button(
                        onClick = onStartScan,
                        colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary)
                    ) {
                        Text("Scan")
                    }
                }
            }

            // Live progress stats banner
            if (isScanning || progress.totalFiles > 0) {
                Card(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
                ) {
                    Column(modifier = Modifier.padding(12.dp)) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween
                        ) {
                            Text(
                                "Total: ${formatBytes(progress.totalBytes)}",
                                style = MaterialTheme.typography.titleMedium,
                                color = MaterialTheme.colorScheme.primary
                            )
                            Text(
                                "${progress.totalFiles} files",
                                style = MaterialTheme.typography.titleMedium
                            )
                        }
                        Spacer(modifier = Modifier.height(4.dp))
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween
                        ) {
                            Text(
                                "${String.format("%.1f", progress.filesPerSec)} files/s • ${formatBytes(progress.bytesPerSec.toLong())}/s",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                            Text(
                                "Workers: ${progress.activeWorkers} • ${progress.elapsedMillis}ms",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                        if (isScanning) {
                            Spacer(modifier = Modifier.height(6.dp))
                            LinearProgressIndicator(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(3.dp)
                                    .clip(RoundedCornerShape(2.dp))
                            )
                        }
                    }
                }
            }

            // Navigation Tabs
            TabRow(
                selectedTabIndex = selectedTab,
                containerColor = MaterialTheme.colorScheme.surface,
                contentColor = MaterialTheme.colorScheme.primary
            ) {
                Tab(
                    selected = selectedTab == 0,
                    onClick = { selectedTab = 0 },
                    text = { Text("Treemap") }
                )
                Tab(
                    selected = selectedTab == 1,
                    onClick = { selectedTab = 1 },
                    text = { Text("Directory Tree") }
                )
                Tab(
                    selected = selectedTab == 2,
                    onClick = { selectedTab = 2 },
                    text = { Text("File Types") }
                )
            }

            // Content Area
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
            ) {
                when (selectedTab) {
                    0 -> {
                        if (treemapNodes.isEmpty()) {
                            Box(
                                modifier = Modifier.fillMaxSize(),
                                contentAlignment = Alignment.Center
                            ) {
                                Text(
                                    if (isScanning) "Building treemap..." else "Tap 'Scan' to visualize disk space",
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
                                    onNodeSelected = { selectedNode = it },
                                    modifier = Modifier.weight(1f)
                                )
                            }
                        }
                    }
                    1 -> {
                        DirectoryList(
                            nodes = treemapNodes,
                            selectedNode = selectedNode,
                            onNodeClick = { selectedNode = it }
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
