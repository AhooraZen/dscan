package com.dscan.app

import androidx.compose.ui.geometry.Rect
import com.dscan.app.ui.SquarifyLayout
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class TreemapLayoutTest {

    @Test
    fun testEmptyNodesLayout() {
        val rects = SquarifyLayout.layout(
            nodes = emptyList(),
            rootId = 0,
            bounds = Rect(0f, 0f, 800f, 600f)
        )
        assertTrue(rects.isEmpty())
    }

    @Test
    fun testSingleRootNodeLayout() {
        val root = TreemapNode(
            id = 0,
            parentId = -1,
            name = "root",
            totalBytes = 1000L,
            directBytes = 1000L,
            relDepth = 0,
            isDir = true,
            extension = "",
            childrenIds = emptyList()
        )
        val rects = SquarifyLayout.layout(
            nodes = listOf(root),
            rootId = 0,
            bounds = Rect(0f, 0f, 800f, 600f)
        )
        assertEquals(1, rects.size)
        assertEquals(0f, rects[0].rect.left)
        assertEquals(0f, rects[0].rect.top)
        assertEquals(800f, rects[0].rect.right)
        assertEquals(600f, rects[0].rect.bottom)
    }

    @Test
    fun testSquarifiedBoundingContainment() {
        val root = TreemapNode(
            id = 0,
            parentId = -1,
            name = "root",
            totalBytes = 10000L,
            directBytes = 0L,
            relDepth = 0,
            isDir = true,
            extension = "",
            childrenIds = listOf(1, 2, 3)
        )
        val c1 = TreemapNode(
            id = 1,
            parentId = 0,
            name = "c1",
            totalBytes = 5000L,
            directBytes = 5000L,
            relDepth = 1,
            isDir = false,
            extension = "zip",
            childrenIds = emptyList()
        )
        val c2 = TreemapNode(
            id = 2,
            parentId = 0,
            name = "c2",
            totalBytes = 3000L,
            directBytes = 3000L,
            relDepth = 1,
            isDir = false,
            extension = "tar",
            childrenIds = emptyList()
        )
        val c3 = TreemapNode(
            id = 3,
            parentId = 0,
            name = "c3",
            totalBytes = 2000L,
            directBytes = 2000L,
            relDepth = 1,
            isDir = false,
            extension = "mp4",
            childrenIds = emptyList()
        )

        val nodes = listOf(root, c1, c2, c3)
        val bounds = Rect(0f, 0f, 1000f, 1000f)
        val rects = SquarifyLayout.layout(
            nodes = nodes,
            rootId = 0,
            bounds = bounds,
            maxDepth = 2,
            minPixelSize = 1f
        )

        assertTrue(rects.isNotEmpty())
        for (item in rects) {
            assertTrue(item.rect.left >= bounds.left - 0.01f)
            assertTrue(item.rect.top >= bounds.top - 0.01f)
            assertTrue(item.rect.right <= bounds.right + 0.01f)
            assertTrue(item.rect.bottom <= bounds.bottom + 0.01f)
        }
    }
}
