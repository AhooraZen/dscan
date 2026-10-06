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
import kotlin.math.min

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
            nodes.filter { it.id != root.id && it.totalBytes > 0 && it.relDepth == 1 }
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

/**
 * Squarified treemap layout (Bruls-Huizing-van Wijk).
 * Items must be pre-sorted descending by totalBytes.
 */
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

    // Area each item occupies in pixels
    val totalArea = w * h
    val areas = FloatArray(items.size) { (items[it].totalBytes.toFloat() / totalWeight) * totalArea }

    var cx = x; var cy = y; var cw = w; var ch = h
    var i = 0

    while (i < items.size) {
        val shortSide = min(cw, ch)
        val row = mutableListOf<Int>()
        var rowArea = 0f

        // Greedily add items while worst aspect ratio improves
        row.add(i)
        rowArea += areas[i]
        var bestWorst = worstAspect(row, areas, rowArea, shortSide)

        var j = i + 1
        while (j < items.size) {
            val testArea = rowArea + areas[j]
            row.add(j)
            val testWorst = worstAspect(row, areas, testArea, shortSide)
            if (testWorst > bestWorst) {
                // Adding this item worsened ratio — remove and stop
                row.removeAt(row.size - 1)
                break
            }
            rowArea = testArea
            bestWorst = testWorst
            j++
        }

        // Lay out this row along the short side
        val layoutHorizontal = cw >= ch

        if (layoutHorizontal) {
            val actualRowWidth = (rowArea / ch).coerceIn(0f, cw)
            var posY = cy
            for (idx in row) {
                val itemH = if (actualRowWidth > 0f) areas[idx] / actualRowWidth else 0f
                outRects.add(LayoutRect(items[idx], cx, posY, actualRowWidth, itemH))
                posY += itemH
            }
            cx += actualRowWidth
            cw -= actualRowWidth
        } else {
            val actualRowHeight = (rowArea / cw).coerceIn(0f, ch)
            var posX = cx
            for (idx in row) {
                val itemW = if (actualRowHeight > 0f) areas[idx] / actualRowHeight else 0f
                outRects.add(LayoutRect(items[idx], posX, cy, itemW, actualRowHeight))
                posX += itemW
            }
            cy += actualRowHeight
            ch -= actualRowHeight
        }

        // Recalculate totalArea for remaining items
        i = j.coerceAtLeast(i + row.size)
    }
}

/** Worst (max) aspect ratio in a row. Lower is better (1.0 = perfect square). */
private fun worstAspect(row: List<Int>, areas: FloatArray, rowArea: Float, side: Float): Float {
    if (rowArea <= 0f || side <= 0f) return Float.MAX_VALUE
    val s2 = side * side
    var worst = 0f
    for (idx in row) {
        val a = areas[idx]
        // aspect = max(s²·a / rowArea², rowArea² / (s²·a))
        val r1 = (s2 * a) / (rowArea * rowArea)
        val r2 = (rowArea * rowArea) / (s2 * a)
        worst = max(worst, max(r1, r2))
    }
    return worst
}
