package com.dscan.app.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import com.dscan.app.TreemapNode
import kotlin.math.max

data class LayoutRect(
    val node: TreemapNode,
    val x: Float,
    val y: Float,
    val width: Float,
    val height: Float
)

@Composable
fun TreemapCanvas(
    nodes: List<TreemapNode>,
    selectedNode: TreemapNode?,
    onNodeSelected: (TreemapNode) -> Unit,
    modifier: Modifier = Modifier
) {
    val outlineColor = MaterialTheme.colorScheme.outline.copy(alpha = 0.4f)
    val selectionColor = MaterialTheme.colorScheme.primary

    // Compute top-level direct items from nodes
    var layoutRects by remember { mutableStateOf<List<LayoutRect>>(emptyList()) }

    Canvas(
        modifier = modifier
            .fillMaxSize()
            .pointerInput(nodes) {
                detectTapGestures { tapOffset ->
                    val hit = layoutRects.find { rect ->
                        tapOffset.x >= rect.x && tapOffset.x <= rect.x + rect.width &&
                                tapOffset.y >= rect.y && tapOffset.y <= rect.y + rect.height
                    }
                    if (hit != null) {
                        onNodeSelected(hit.node)
                    }
                }
            }
    ) {
        val w = size.width
        val h = size.height
        if (w <= 0f || h <= 0f || nodes.isEmpty()) return@Canvas

        // Find direct children of root or top elements
        val root = nodes.firstOrNull() ?: return@Canvas
        val children = if (root.childrenIds.isNotEmpty()) {
            val childIdSet = root.childrenIds.toSet()
            nodes.filter { childIdSet.contains(it.id) && it.totalBytes > 0 }
        } else {
            nodes.filter { it.id != root.id && it.totalBytes > 0 }
        }.sortedByDescending { it.totalBytes }

        if (children.isEmpty()) return@Canvas

        val totalWeight = children.sumOf { it.totalBytes }.coerceAtLeast(1L).toFloat()
        val rects = mutableListOf<LayoutRect>()

        // Squarified treemap layout algorithm
        squarify(
            items = children,
            x = 0f,
            y = 0f,
            w = w,
            h = h,
            totalWeight = totalWeight,
            outRects = rects
        )

        layoutRects = rects

        // Render rectangles
        rects.forEach { rect ->
            if (rect.width < 1f || rect.height < 1f) return@forEach

            val baseColor = if (rect.node.isDir) {
                Color(0xFF334155)
            } else {
                getExtensionColor(rect.node.extension)
            }

            // Draw filled rect
            drawRect(
                color = baseColor,
                topLeft = Offset(rect.x, rect.y),
                size = Size(rect.width, rect.height)
            )

            // Draw cushion top/left highlight
            if (rect.width > 6f && rect.height > 6f) {
                drawRect(
                    color = Color.White.copy(alpha = 0.12f),
                    topLeft = Offset(rect.x + 1f, rect.y + 1f),
                    size = Size(rect.width - 2f, max(1f, rect.height * 0.25f))
                )
            }

            // Draw border
            val isSelected = selectedNode?.id == rect.node.id
            drawRect(
                color = if (isSelected) selectionColor else outlineColor,
                topLeft = Offset(rect.x, rect.y),
                size = Size(rect.width, rect.height),
                style = Stroke(width = if (isSelected) 3f else 1f)
            )
        }
    }
}

private fun squarify(
    items: List<TreemapNode>,
    x: Float,
    y: Float,
    w: Float,
    h: Float,
    totalWeight: Float,
    outRects: MutableList<LayoutRect>
) {
    if (items.isEmpty() || w <= 0f || h <= 0f || totalWeight <= 0f) return

    val isHorizontal = w >= h
    var currentX = x
    var currentY = y

    // Slice and dice partitioning
    items.forEach { item ->
        val fraction = (item.totalBytes.toFloat() / totalWeight).coerceIn(0f, 1f)
        if (isHorizontal) {
            val itemWidth = w * fraction
            if (itemWidth > 0.5f) {
                outRects.add(LayoutRect(item, currentX, y, itemWidth, h))
                currentX += itemWidth
            }
        } else {
            val itemHeight = h * fraction
            if (itemHeight > 0.5f) {
                outRects.add(LayoutRect(item, x, currentY, w, itemHeight))
                currentY += itemHeight
            }
        }
    }
}
