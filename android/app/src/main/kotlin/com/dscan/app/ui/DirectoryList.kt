package com.dscan.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import com.dscan.app.ExtensionStat
import com.dscan.app.TreemapNode

@Composable
fun DirectoryList(
    nodes: List<TreemapNode>,
    selectedNode: TreemapNode?,
    onNodeClick: (TreemapNode) -> Unit,
    modifier: Modifier = Modifier
) {
    val itemsToDisplay = if (selectedNode != null && selectedNode.childrenIds.isNotEmpty()) {
        val childSet = selectedNode.childrenIds.toSet()
        nodes.filter { childSet.contains(it.id) }
    } else {
        nodes.filter { it.relDepth == 1 }
    }.sortedByDescending { it.totalBytes }

    val totalBytes = (selectedNode?.totalBytes ?: nodes.firstOrNull()?.totalBytes ?: 1L).coerceAtLeast(1L)

    LazyColumn(
        modifier = modifier.fillMaxWidth(),
        contentPadding = PaddingValues(vertical = 8.dp)
    ) {
        items(itemsToDisplay, key = { it.id }) { node ->
            val fraction = (node.totalBytes.toFloat() / totalBytes.toFloat()).coerceIn(0f, 1f)
            val isSelected = selectedNode?.id == node.id

            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 4.dp)
                    .clickable { onNodeClick(node) },
                colors = CardDefaults.cardColors(
                    containerColor = if (isSelected) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surface
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
                            Box(
                                modifier = Modifier
                                    .size(10.dp)
                                    .clip(RoundedCornerShape(2.dp))
                                    .background(if (node.isDir) MaterialTheme.colorScheme.outline else getExtensionColor(node.extension))
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Text(
                                text = node.name,
                                style = MaterialTheme.typography.bodyMedium,
                                maxLines = 1
                            )
                        }
                        Text(
                            text = formatBytes(node.totalBytes),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.primary
                        )
                    }

                    Spacer(modifier = Modifier.height(6.dp))

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

@Composable
fun ExtensionBreakdownList(
    stats: List<ExtensionStat>,
    modifier: Modifier = Modifier
) {
    LazyColumn(
        modifier = modifier.fillMaxWidth(),
        contentPadding = PaddingValues(vertical = 8.dp)
    ) {
        items(stats, key = { it.extension }) { stat ->
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 4.dp),
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
                            Box(
                                modifier = Modifier
                                    .size(10.dp)
                                    .clip(RoundedCornerShape(2.dp))
                                    .background(getExtensionColor(stat.extension))
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Text(
                                text = if (stat.extension.isEmpty()) "[no extension]" else stat.extension,
                                style = MaterialTheme.typography.bodyMedium
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
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.primary
                        )
                    }

                    Spacer(modifier = Modifier.height(6.dp))

                    LinearProgressIndicator(
                        progress = { (stat.percentage / 100f).coerceIn(0f, 1f) },
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(4.dp)
                            .clip(RoundedCornerShape(2.dp)),
                        color = getExtensionColor(stat.extension),
                        trackColor = MaterialTheme.colorScheme.surfaceVariant
                    )
                }
            }
        }
    }
}
