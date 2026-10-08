package com.dscan.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.InsertDriveFile
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.dscan.app.ExtensionStat
import com.dscan.app.TreemapNode

/**
 * Hierarchical directory explorer with breadcrumb navigation and proportional size bars.
 */
@Composable
fun DirectoryExplorer(
    nodes: List<TreemapNode>,
    onNodeSelected: (TreemapNode) -> Unit = {},
    modifier: Modifier = Modifier
) {
    if (nodes.isEmpty()) {
        Box(
            modifier = modifier.fillMaxSize(),
            contentAlignment = Alignment.Center
        ) {
            Text(
                "No directory data available. Run a scan first.",
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        return
    }

    val nodeMap = remember(nodes) { nodes.associateBy { it.id } }
    val rootNode = remember(nodes) { nodes.firstOrNull { it.parentId == -1 } ?: nodes.firstOrNull() }

    var currentFolderId by rememberSaveable(nodes) {
        mutableIntStateOf(rootNode?.id ?: 0)
    }

    val currentFolder = nodeMap[currentFolderId] ?: rootNode ?: return

    // Reconstruct breadcrumb trail up to the root
    val breadcrumbs = remember(currentFolder, nodeMap) {
        val chain = mutableListOf<TreemapNode>()
        var curr: TreemapNode? = currentFolder
        val visited = mutableSetOf<Int>()
        while (curr != null && !visited.contains(curr.id)) {
            visited.add(curr.id)
            chain.add(0, curr)
            if (curr.parentId == -1 || curr.parentId == curr.id) break
            curr = nodeMap[curr.parentId]
        }
        chain
    }

    // Direct children of current folder
    val children = remember(currentFolder, nodes) {
        val childIds = currentFolder.childrenIds.toSet()
        val direct = if (childIds.isNotEmpty()) {
            nodes.filter { childIds.contains(it.id) }
        } else {
            nodes.filter { it.parentId == currentFolder.id && it.id != currentFolder.id }
        }
        val dirs = direct.filter { it.isDir }.sortedByDescending { it.totalBytes }
        val files = direct.filter { !it.isDir }.sortedByDescending { it.totalBytes }
        dirs + files
    }

    val parentBytes = currentFolder.totalBytes.coerceAtLeast(1L)
    val breadcrumbScrollState = rememberScrollState()

    LaunchedEffect(breadcrumbs.size) {
        breadcrumbScrollState.animateScrollTo(breadcrumbScrollState.maxValue)
    }

    Column(modifier = modifier.fillMaxSize()) {
        // Breadcrumb Navigation Bar
        Surface(
            color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
            modifier = Modifier.fillMaxWidth()
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 12.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                // Back / Up Button
                val parentId = currentFolder.parentId
                if (parentId != -1 && parentId != currentFolder.id && nodeMap.containsKey(parentId)) {
                    IconButton(
                        onClick = {
                            currentFolderId = parentId
                        },
                        modifier = Modifier.size(36.dp)
                    ) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Navigate Up",
                            tint = MaterialTheme.colorScheme.primary
                        )
                    }
                    Spacer(modifier = Modifier.width(4.dp))
                } else {
                    Icon(
                        Icons.Default.Folder,
                        contentDescription = "Root Folder",
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(24.dp)
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                }

                // Breadcrumb trail
                Row(
                    modifier = Modifier
                        .weight(1f)
                        .horizontalScroll(breadcrumbScrollState),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    breadcrumbs.forEachIndexed { index, crumb ->
                        val isLast = index == breadcrumbs.lastIndex
                        Text(
                            text = if (crumb.name.isEmpty()) "Root" else crumb.name,
                            style = MaterialTheme.typography.bodyMedium.copy(
                                fontWeight = if (isLast) FontWeight.Bold else FontWeight.Normal
                            ),
                            color = if (isLast) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier
                                .clip(RoundedCornerShape(4.dp))
                                .clickable(!isLast) {
                                    currentFolderId = crumb.id
                                }
                                .padding(horizontal = 4.dp, vertical = 2.dp)
                        )
                        if (!isLast) {
                            Text(
                                text = " / ",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.outline
                            )
                        }
                    }
                }
            }
        }

        // Folder contents list
        if (children.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    "This directory is empty or has no scanned sub-items",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    style = MaterialTheme.typography.bodyMedium
                )
            }
        } else {
            LazyColumn(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f),
                contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                items(children, key = { it.id }) { item ->
                    val fraction = (item.totalBytes.toFloat() / parentBytes.toFloat()).coerceIn(0f, 1f)
                    val percentStr = String.format("%.1f%%", fraction * 100f)

                    Card(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(12.dp))
                            .clickable {
                                onNodeSelected(item)
                                if (item.isDir) {
                                    currentFolderId = item.id
                                }
                            },
                        colors = CardDefaults.cardColors(
                            containerColor = MaterialTheme.colorScheme.surface
                        )
                    ) {
                        Column(modifier = Modifier.padding(12.dp)) {
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    modifier = Modifier.weight(1f)
                                ) {
                                    if (item.isDir) {
                                        Box(
                                            modifier = Modifier
                                                .size(32.dp)
                                                .clip(RoundedCornerShape(8.dp))
                                                .background(MaterialTheme.colorScheme.primaryContainer),
                                            contentAlignment = Alignment.Center
                                        ) {
                                            Icon(
                                                Icons.Default.Folder,
                                                contentDescription = null,
                                                tint = MaterialTheme.colorScheme.primary,
                                                modifier = Modifier.size(18.dp)
                                            )
                                        }
                                    } else {
                                        Box(
                                            modifier = Modifier
                                                .size(32.dp)
                                                .clip(RoundedCornerShape(8.dp))
                                                .background(getExtensionColor(item.extension).copy(alpha = 0.2f)),
                                            contentAlignment = Alignment.Center
                                        ) {
                                            Icon(
                                                Icons.AutoMirrored.Filled.InsertDriveFile,
                                                contentDescription = null,
                                                tint = getExtensionColor(item.extension),
                                                modifier = Modifier.size(18.dp)
                                            )
                                        }
                                    }

                                    Spacer(modifier = Modifier.width(10.dp))

                                    Column(modifier = Modifier.weight(1f)) {
                                        Text(
                                            text = item.name,
                                            style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.SemiBold),
                                            maxLines = 1,
                                            overflow = TextOverflow.Ellipsis
                                        )
                                        if (item.isDir && item.childrenIds.isNotEmpty()) {
                                            Text(
                                                text = "${item.childrenIds.size} sub-items",
                                                style = MaterialTheme.typography.bodySmall,
                                                color = MaterialTheme.colorScheme.onSurfaceVariant
                                            )
                                        }
                                    }
                                }

                                Spacer(modifier = Modifier.width(8.dp))

                                Column(horizontalAlignment = Alignment.End) {
                                    Text(
                                        text = formatBytes(item.totalBytes),
                                        style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Bold),
                                        color = MaterialTheme.colorScheme.primary
                                    )
                                    Text(
                                        text = percentStr,
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.outline
                                    )
                                }
                            }

                            Spacer(modifier = Modifier.height(8.dp))

                            // Proportional bar
                            LinearProgressIndicator(
                                progress = { fraction },
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(4.dp)
                                    .clip(RoundedCornerShape(2.dp)),
                                color = if (item.isDir) MaterialTheme.colorScheme.primary else getExtensionColor(item.extension),
                                trackColor = MaterialTheme.colorScheme.surfaceVariant
                            )
                        }
                    }
                }
            }
        }
    }
}

/**
 * Storage Hogs: Ranks the largest space consumers across the scanned tree.
 */
@Composable
fun StorageHogsList(
    nodes: List<TreemapNode>,
    onNodeSelected: (TreemapNode) -> Unit = {},
    modifier: Modifier = Modifier
) {
    if (nodes.isEmpty()) {
        Box(
            modifier = modifier.fillMaxSize(),
            contentAlignment = Alignment.Center
        ) {
            Text(
                "No scanned files to analyze.",
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        return
    }

    // Rank top largest individual files or directories
    val hogs = remember(nodes) {
        val nonRoots = nodes.filter { it.relDepth > 0 }
        val candidateFiles = nonRoots.filter { !it.isDir }
        val candidates = if (candidateFiles.isNotEmpty()) candidateFiles else nonRoots
        candidates.sortedByDescending { it.totalBytes }.take(50)
    }

    val totalBytes = remember(nodes) {
        (nodes.firstOrNull()?.totalBytes ?: 1L).coerceAtLeast(1L)
    }

    LazyColumn(
        modifier = modifier.fillMaxWidth(),
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        itemsIndexed(hogs, key = { _, node -> node.id }) { index, node ->
            val fraction = (node.totalBytes.toFloat() / totalBytes.toFloat()).coerceIn(0f, 1f)
            val rank = index + 1
            val isTop3 = rank <= 3

            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(12.dp))
                    .clickable { onNodeSelected(node) },
                colors = CardDefaults.cardColors(
                    containerColor = if (isTop3) MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.35f) else MaterialTheme.colorScheme.surface
                )
            ) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier.weight(1f)
                        ) {
                            // Rank Badge
                            Box(
                                modifier = Modifier
                                    .size(28.dp)
                                    .clip(CircleShape)
                                    .background(
                                        if (isTop3) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceVariant
                                    ),
                                contentAlignment = Alignment.Center
                            ) {
                                Text(
                                    text = "#$rank",
                                    style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Bold),
                                    color = if (isTop3) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }

                            Spacer(modifier = Modifier.width(10.dp))

                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = node.name,
                                    style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.SemiBold),
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis
                                )
                                Text(
                                    text = if (node.isDir) "Directory" else if (node.extension.isNotEmpty()) ".${node.extension}" else "File",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                        }

                        Spacer(modifier = Modifier.width(8.dp))

                        Column(horizontalAlignment = Alignment.End) {
                            Text(
                                text = formatBytes(node.totalBytes),
                                style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Bold),
                                color = MaterialTheme.colorScheme.primary
                            )
                            Text(
                                text = String.format("%.2f%% of scan", fraction * 100f),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.outline
                            )
                        }
                    }

                    Spacer(modifier = Modifier.height(8.dp))

                    LinearProgressIndicator(
                        progress = { fraction },
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(4.dp)
                            .clip(RoundedCornerShape(2.dp)),
                        color = if (node.isDir) MaterialTheme.colorScheme.secondary else getExtensionColor(node.extension),
                        trackColor = MaterialTheme.colorScheme.surfaceVariant
                    )
                }
            }
        }
    }
}

/**
 * File Types breakdown list.
 */
@Composable
fun ExtensionBreakdownList(
    stats: List<ExtensionStat>,
    modifier: Modifier = Modifier
) {
    if (stats.isEmpty()) {
        Box(
            modifier = modifier.fillMaxSize(),
            contentAlignment = Alignment.Center
        ) {
            Text(
                "No file type stats available.",
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        return
    }

    LazyColumn(
        modifier = modifier.fillMaxWidth(),
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        items(stats, key = { it.extension }) { stat ->
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(12.dp)),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surface
                )
            ) {
                Column(modifier = Modifier.padding(14.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier.weight(1f)
                        ) {
                            Box(
                                modifier = Modifier
                                    .size(14.dp)
                                    .clip(RoundedCornerShape(4.dp))
                                    .background(getExtensionColor(stat.extension))
                            )
                            Spacer(modifier = Modifier.width(10.dp))
                            Text(
                                text = if (stat.extension.isEmpty()) "[no extension]" else ".${stat.extension}",
                                style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.SemiBold)
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Text(
                                text = "(${stat.fileCount} files)",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                        Text(
                            text = "${formatBytes(stat.totalBytes)} (${String.format("%.1f%%", stat.percentage)})",
                            style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Bold),
                            color = MaterialTheme.colorScheme.primary
                        )
                    }

                    Spacer(modifier = Modifier.height(8.dp))

                    LinearProgressIndicator(
                        progress = { (stat.percentage / 100f).coerceIn(0f, 1f) },
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(5.dp)
                            .clip(RoundedCornerShape(2.5.dp)),
                        color = getExtensionColor(stat.extension),
                        trackColor = MaterialTheme.colorScheme.surfaceVariant
                    )
                }
            }
        }
    }
}
