package com.dscan.app.ui

import androidx.compose.animation.*
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import com.dscan.app.DscanBridge
import com.dscan.app.TreemapNode
import kotlin.math.max
import kotlin.math.min

data class LayoutRect(
    val node: TreemapNode,
    val rect: Rect,
    val isLeaf: Boolean
)

object SquarifyLayout {
    fun layout(
        nodes: List<TreemapNode>,
        rootId: Int,
        bounds: Rect,
        maxDepth: Int = 4,
        minPixelSize: Float = 4f
    ): List<LayoutRect> {
        if (nodes.isEmpty() || bounds.width <= 0f || bounds.height <= 0f) return emptyList()

        val nodeMap = nodes.associateBy { it.id }
        val root = nodeMap[rootId] ?: nodes.firstOrNull { it.parentId == -1 } ?: nodes.firstOrNull() ?: return emptyList()

        val result = mutableListOf<LayoutRect>()
        layoutRecursive(root, bounds, nodeMap, nodes, 0, maxDepth, minPixelSize, result)
        return result
    }

    private fun layoutRecursive(
        current: TreemapNode,
        bounds: Rect,
        nodeMap: Map<Int, TreemapNode>,
        allNodes: List<TreemapNode>,
        depth: Int,
        maxDepth: Int,
        minPixelSize: Float,
        out: MutableList<LayoutRect>
    ) {
        if (bounds.width < minPixelSize || bounds.height < minPixelSize) return

        val childIds = current.childrenIds.toSet()
        val children = if (childIds.isNotEmpty()) {
            allNodes.filter { childIds.contains(it.id) && it.totalBytes > 0 }
        } else {
            allNodes.filter { it.parentId == current.id && it.id != current.id && it.totalBytes > 0 }
        }.sortedByDescending { it.totalBytes }

        if (children.isEmpty() || depth >= maxDepth) {
            out.add(LayoutRect(current, bounds, isLeaf = true))
            return
        }

        // Parent container rect
        out.add(LayoutRect(current, bounds, isLeaf = false))

        val totalWeight = children.sumOf { it.totalBytes }.coerceAtLeast(1L).toDouble()
        val containerArea = (bounds.width * bounds.height).toDouble()

        // Normalize weights to areas
        val itemAreas = children.map { (it.totalBytes.toDouble() / totalWeight) * containerArea }

        squarify(
            children = children,
            areas = itemAreas,
            bounds = bounds,
            nodeMap = nodeMap,
            allNodes = allNodes,
            depth = depth + 1,
            maxDepth = maxDepth,
            minPixelSize = minPixelSize,
            out = out
        )
    }

    private fun squarify(
        children: List<TreemapNode>,
        areas: List<Double>,
        bounds: Rect,
        nodeMap: Map<Int, TreemapNode>,
        allNodes: List<TreemapNode>,
        depth: Int,
        maxDepth: Int,
        minPixelSize: Float,
        out: MutableList<LayoutRect>
    ) {
        if (children.isEmpty() || bounds.width <= 0f || bounds.height <= 0f) return

        var remainingBounds = bounds
        var rowChildren = mutableListOf<TreemapNode>()
        var rowAreas = mutableListOf<Double>()
        var remainingChildren = children
        var remainingAreas = areas

        while (remainingChildren.isNotEmpty()) {
            val isHorizontal = remainingBounds.width >= remainingBounds.height
            val sideLength = if (isHorizontal) remainingBounds.height.toDouble() else remainingBounds.width.toDouble()

            val nextChild = remainingChildren.first()
            val nextArea = remainingAreas.first()

            val testRowAreas = ArrayList(rowAreas).apply { add(nextArea) }

            if (rowAreas.isEmpty() || worstAspectRatio(testRowAreas, sideLength) <= worstAspectRatio(rowAreas, sideLength)) {
                rowChildren.add(nextChild)
                rowAreas.add(nextArea)
                remainingChildren = remainingChildren.drop(1)
                remainingAreas = remainingAreas.drop(1)
            } else {
                // Layout current row and shrink remaining bounds
                remainingBounds = layoutRow(
                    rowChildren,
                    rowAreas,
                    remainingBounds,
                    sideLength,
                    isHorizontal,
                    nodeMap,
                    allNodes,
                    depth,
                    maxDepth,
                    minPixelSize,
                    out
                )
                rowChildren = mutableListOf()
                rowAreas = mutableListOf()
            }
        }

        if (rowChildren.isNotEmpty()) {
            val isHorizontal = remainingBounds.width >= remainingBounds.height
            val sideLength = if (isHorizontal) remainingBounds.height.toDouble() else remainingBounds.width.toDouble()
            layoutRow(
                rowChildren,
                rowAreas,
                remainingBounds,
                sideLength,
                isHorizontal,
                nodeMap,
                allNodes,
                depth,
                maxDepth,
                minPixelSize,
                out
            )
        }
    }

    private fun worstAspectRatio(rowAreas: List<Double>, sideLength: Double): Double {
        if (rowAreas.isEmpty() || sideLength <= 0.0) return Double.MAX_VALUE
        val sum = rowAreas.sum()
        if (sum <= 0.0) return Double.MAX_VALUE
        val sideSq = sideLength * sideLength
        val sumSq = sum * sum

        var worst = 0.0
        for (area in rowAreas) {
            if (area <= 0.0) continue
            val r1 = (sideSq * area) / sumSq
            val r2 = sumSq / (sideSq * area)
            val aspect = max(r1, r2)
            if (aspect > worst) {
                worst = aspect
            }
        }
        return worst
    }

    private fun layoutRow(
        rowChildren: List<TreemapNode>,
        rowAreas: List<Double>,
        bounds: Rect,
        sideLength: Double,
        isHorizontal: Boolean,
        nodeMap: Map<Int, TreemapNode>,
        allNodes: List<TreemapNode>,
        depth: Int,
        maxDepth: Int,
        minPixelSize: Float,
        out: MutableList<LayoutRect>
    ): Rect {
        val rowSum = rowAreas.sum()
        if (rowSum <= 0.0 || sideLength <= 0.0) return bounds

        val rowThickness = (rowSum / sideLength).toFloat()

        var currentOffset = if (isHorizontal) bounds.top else bounds.left

        for (i in rowChildren.indices) {
            val child = rowChildren[i]
            val area = rowAreas[i]
            val itemLength = ((area / rowSum) * sideLength).toFloat()

            val itemRect = if (isHorizontal) {
                Rect(
                    left = bounds.left,
                    top = currentOffset,
                    right = bounds.left + rowThickness,
                    bottom = currentOffset + itemLength
                )
            } else {
                Rect(
                    left = currentOffset,
                    top = bounds.top,
                    right = currentOffset + itemLength,
                    bottom = bounds.top + rowThickness
                )
            }

            currentOffset += itemLength

            layoutRecursive(
                current = child,
                bounds = itemRect,
                nodeMap = nodeMap,
                allNodes = allNodes,
                depth = depth,
                maxDepth = maxDepth,
                minPixelSize = minPixelSize,
                out = out
            )
        }

        return if (isHorizontal) {
            Rect(
                left = bounds.left + rowThickness,
                top = bounds.top,
                right = bounds.right,
                bottom = bounds.bottom
            )
        } else {
            Rect(
                left = bounds.left,
                top = bounds.top + rowThickness,
                right = bounds.right,
                bottom = bounds.bottom
            )
        }
    }
}

@Composable
fun TreemapView(
    nodes: List<TreemapNode>,
    targetPath: String,
    searchQuery: String = "",
    onNodeSelected: (TreemapNode) -> Unit = {},
    modifier: Modifier = Modifier
) {
    if (nodes.isEmpty()) {
        Box(
            modifier = modifier.fillMaxSize(),
            contentAlignment = Alignment.Center
        ) {
            Text(
                "No treemap data available. Start a scan to visualize disk usage.",
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        return
    }

    val nodeMap = remember(nodes) { nodes.associateBy { it.id } }
    val defaultRoot = remember(nodes) { nodes.firstOrNull { it.parentId == -1 } ?: nodes.firstOrNull() }

    var currentRootId by remember(nodes) { mutableIntStateOf(defaultRoot?.id ?: 0) }
    val currentRoot = nodeMap[currentRootId] ?: defaultRoot ?: return

    var hoveredRect by remember { mutableStateOf<LayoutRect?>(null) }
    var hoverPosition by remember { mutableStateOf<Offset?>(null) }

    var contextMenuNode by remember { mutableStateOf<TreemapNode?>(null) }
    var contextMenuOffset by remember { mutableStateOf<Offset?>(null) }

    // Breadcrumbs chain
    val breadcrumbs = remember(currentRoot, nodeMap) {
        val chain = mutableListOf<TreemapNode>()
        var curr: TreemapNode? = currentRoot
        val visited = mutableSetOf<Int>()
        while (curr != null && !visited.contains(curr.id)) {
            visited.add(curr.id)
            chain.add(0, curr)
            if (curr.parentId == -1 || curr.parentId == curr.id) break
            curr = nodeMap[curr.parentId]
        }
        chain
    }

    val breadcrumbScrollState = rememberScrollState()
    LaunchedEffect(breadcrumbs.size) {
        breadcrumbScrollState.animateScrollTo(breadcrumbScrollState.maxValue)
    }

    Column(modifier = modifier.fillMaxSize()) {
        // Navigation Breadcrumbs Trail
        Surface(
            color = MaterialTheme.colorScheme.surface,
            modifier = Modifier.fillMaxWidth()
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 12.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Icon(
                    Icons.Default.FolderOpen,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(20.dp)
                )
                Spacer(modifier = Modifier.width(8.dp))

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
                                    currentRootId = crumb.id
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

                if (currentRoot.id != (defaultRoot?.id ?: 0)) {
                    TextButton(
                        onClick = { currentRootId = defaultRoot?.id ?: 0 },
                        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp)
                    ) {
                        Icon(Icons.Default.ZoomOutMap, contentDescription = null, modifier = Modifier.size(16.dp))
                        Spacer(modifier = Modifier.width(4.dp))
                        Text("Reset Zoom", style = MaterialTheme.typography.labelSmall)
                    }
                }
            }
        }

        HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f))

        // Treemap Canvas Visualizer
        BoxWithConstraints(
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
                .background(MaterialTheme.colorScheme.background)
        ) {
            val canvasWidth = constraints.maxWidth.toFloat()
            val canvasHeight = constraints.maxHeight.toFloat()

            val layoutRects = remember(nodes, currentRootId, canvasWidth, canvasHeight) {
                if (canvasWidth > 0f && canvasHeight > 0f) {
                    SquarifyLayout.layout(
                        nodes = nodes,
                        rootId = currentRootId,
                        bounds = Rect(0f, 0f, canvasWidth, canvasHeight),
                        maxDepth = 6,
                        minPixelSize = 3f
                    )
                } else emptyList()
            }

            val leafRects = remember(layoutRects) {
                layoutRects.filter { it.isLeaf }
            }

            Canvas(
                modifier = Modifier
                    .fillMaxSize()
                    .pointerInput(leafRects) {
                        awaitPointerEventScope {
                            while (true) {
                                val event = awaitPointerEvent(PointerEventPass.Main)
                                val change = event.changes.firstOrNull()
                                if (change != null) {
                                    val pos = change.position
                                    hoverPosition = pos
                                    hoveredRect = leafRects.lastOrNull { it.rect.contains(pos) }
                                }
                            }
                        }
                    }
                    .pointerInput(leafRects, currentRootId) {
                        detectTapGestures(
                            onTap = { pos ->
                                val clicked = leafRects.lastOrNull { it.rect.contains(pos) }
                                if (clicked != null) {
                                    onNodeSelected(clicked.node)
                                    if (clicked.node.isDir) {
                                        currentRootId = clicked.node.id
                                    }
                                }
                            },
                            onLongPress = { pos ->
                                val clicked = leafRects.lastOrNull { it.rect.contains(pos) }
                                if (clicked != null) {
                                    contextMenuNode = clicked.node
                                    contextMenuOffset = pos
                                }
                            }
                        )
                    }
            ) {
                drawTreemap(
                    rects = leafRects,
                    hoveredNodeId = hoveredRect?.node?.id,
                    searchQuery = searchQuery,
                    totalRootBytes = currentRoot.totalBytes
                )
            }

            // Floating Hover Tooltip
            hoveredRect?.let { hr ->
                hoverPosition?.let { pos ->
                    val node = hr.node
                    val share = if (currentRoot.totalBytes > 0) (node.totalBytes.toDouble() / currentRoot.totalBytes.toDouble()) * 100.0 else 0.0

                    Box(
                        modifier = Modifier
                            .offset {
                                val x = (pos.x + 16).toInt().coerceIn(0, (canvasWidth - 220).toInt().coerceAtLeast(0))
                                val y = (pos.y + 16).toInt().coerceIn(0, (canvasHeight - 90).toInt().coerceAtLeast(0))
                                IntOffset(x, y)
                            }
                            .clip(RoundedCornerShape(8.dp))
                            .background(SurfaceDark.copy(alpha = 0.95f))
                            .border(1.dp, BorderDark, RoundedCornerShape(8.dp))
                            .padding(horizontal = 10.dp, vertical = 8.dp)
                    ) {
                        Column {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Box(
                                    modifier = Modifier
                                        .size(10.dp)
                                        .clip(RoundedCornerShape(2.dp))
                                        .background(getExtensionColor(node.extension))
                                )
                                Spacer(modifier = Modifier.width(6.dp))
                                Text(
                                    text = node.name,
                                    style = MaterialTheme.typography.bodySmall.copy(fontWeight = FontWeight.Bold),
                                    color = Color.White,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis
                                )
                            }
                            Spacer(modifier = Modifier.height(4.dp))
                            Text(
                                text = "${formatBytes(node.totalBytes)} (${String.format("%.2f%%", share)})",
                                style = MaterialTheme.typography.bodySmall,
                                color = AccentCyan
                            )
                            if (node.isDir) {
                                Text(
                                    text = "Directory • Click to zoom in",
                                    style = MaterialTheme.typography.labelSmall,
                                    color = TextMutedDark
                                )
                            }
                        }
                    }
                }
            }

            // Context Menu Popup
            contextMenuNode?.let { node ->
                contextMenuOffset?.let { offset ->
                    Popup(
                        offset = IntOffset(offset.x.toInt(), offset.y.toInt()),
                        onDismissRequest = { contextMenuNode = null }
                    ) {
                        Surface(
                            shape = RoundedCornerShape(8.dp),
                            color = MaterialTheme.colorScheme.surface,
                            border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
                            shadowElevation = 8.dp,
                            modifier = Modifier.width(220.dp)
                        ) {
                            Column(modifier = Modifier.padding(vertical = 4.dp)) {
                                val fullPath = buildFullPath(node, nodeMap, targetPath)

                                DropdownMenuItem(
                                    text = { Text("Reveal in File Manager") },
                                    leadingIcon = { Icon(Icons.Default.FolderOpen, contentDescription = null) },
                                    onClick = {
                                        DscanBridge.revealInFileManager(fullPath)
                                        contextMenuNode = null
                                    }
                                )
                                if (node.isDir) {
                                    DropdownMenuItem(
                                        text = { Text("Zoom In") },
                                        leadingIcon = { Icon(Icons.Default.ZoomIn, contentDescription = null) },
                                        onClick = {
                                            currentRootId = node.id
                                            contextMenuNode = null
                                        }
                                    )
                                }
                                DropdownMenuItem(
                                    text = { Text("Move to Trash", color = MaterialTheme.colorScheme.error) },
                                    leadingIcon = { Icon(Icons.Default.Delete, contentDescription = null, tint = MaterialTheme.colorScheme.error) },
                                    onClick = {
                                        DscanBridge.moveToTrash(fullPath, targetPath)
                                        contextMenuNode = null
                                    }
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

private fun buildFullPath(node: TreemapNode, nodeMap: Map<Int, TreemapNode>, rootPath: String): String {
    val chain = mutableListOf<String>()
    var curr: TreemapNode? = node
    val visited = mutableSetOf<Int>()
    while (curr != null && !visited.contains(curr.id)) {
        visited.add(curr.id)
        if (curr.parentId != -1 && curr.name.isNotEmpty()) {
            chain.add(0, curr.name)
        }
        curr = nodeMap[curr.parentId]
    }
    return if (chain.isEmpty()) rootPath else "$rootPath/${chain.joinToString("/")}"
}

private fun DrawScope.drawTreemap(
    rects: List<LayoutRect>,
    hoveredNodeId: Int?,
    searchQuery: String,
    totalRootBytes: Long
) {
    val queryLower = searchQuery.trim().lowercase()

    for (item in rects) {
        val r = item.rect
        if (r.width <= 0.5f || r.height <= 0.5f) continue

        val node = item.node
        val baseColor = if (node.isDir) {
            Color(0xFF1E293B)
        } else {
            getExtensionColor(node.extension)
        }

        val isHovered = hoveredNodeId == node.id
        val matchesSearch = queryLower.isNotEmpty() && node.name.lowercase().contains(queryLower)
        val isDimmed = queryLower.isNotEmpty() && !matchesSearch

        // 1. Draw base filled rect
        drawRect(
            color = baseColor,
            topLeft = Offset(r.left, r.top),
            size = Size(r.width, r.height)
        )

        // 2. Draw cushion surface effect (top-left highlight and bottom-right shadow)
        if (r.width > 4f && r.height > 4f) {
            // Highlight
            drawRect(
                brush = Brush.linearGradient(
                    colors = listOf(Color.White.copy(alpha = 0.28f), Color.Transparent),
                    start = Offset(r.left, r.top),
                    end = Offset(r.left + min(r.width, 32f), r.top + min(r.height, 32f))
                ),
                topLeft = Offset(r.left, r.top),
                size = Size(r.width, r.height)
            )

            // Shadow
            drawRect(
                brush = Brush.linearGradient(
                    colors = listOf(Color.Transparent, Color.Black.copy(alpha = 0.35f)),
                    start = Offset(r.left + r.width * 0.5f, r.top + r.height * 0.5f),
                    end = Offset(r.right, r.bottom)
                ),
                topLeft = Offset(r.left, r.top),
                size = Size(r.width, r.height)
            )
        }

        // 3. Search dimming
        if (isDimmed) {
            drawRect(
                color = Color.Black.copy(alpha = 0.65f),
                topLeft = Offset(r.left, r.top),
                size = Size(r.width, r.height)
            )
        }

        // 4. Border outline
        val borderColor = when {
            matchesSearch -> AccentCyan
            isHovered -> Color.White
            else -> BorderDark
        }
        val borderWidth = if (matchesSearch || isHovered) 2f else 1f

        drawRect(
            color = borderColor,
            topLeft = Offset(r.left, r.top),
            size = Size(r.width, r.height),
            style = Stroke(width = borderWidth)
        )
    }
}
