package com.dscan.app

import org.json.JSONArray
import org.json.JSONObject

data class ScanProgress(
    val totalBytes: Long = 0L,
    val totalFiles: Long = 0L,
    val activeWorkers: Int = 0,
    val elapsedMillis: Long = 0L,
    val filesPerSec: Double = 0.0,
    val bytesPerSec: Double = 0.0,
    val isComplete: Boolean = false,
    val currentPath: String = ""
)

data class TreemapNode(
    val id: Int,
    val parentId: Int,
    val name: String,
    val totalBytes: Long,
    val directBytes: Long,
    val relDepth: Int,
    val isDir: Boolean,
    val extension: String,
    val childrenIds: List<Int>
)

data class ExtensionStat(
    val extension: String,
    val totalBytes: Long,
    val fileCount: Long,
    val percentage: Float
)

object DscanBridge {
    init {
        try {
            System.loadLibrary("dscan")
        } catch (e: UnsatisfiedLinkError) {
            System.err.println("Failed to load libdscan.so: ${e.message}")
        }
    }

    external fun startScan(path: String, threads: Int): Long
    external fun pollProgress(sessionPtr: Long): String
    external fun getTreemapNodes(sessionPtr: Long, maxDepth: Int, maxNodes: Int): String
    external fun getExtensionBreakdown(sessionPtr: Long, limit: Int): String
    external fun cancelScan(sessionPtr: Long)
    external fun stopScan(sessionPtr: Long)

    fun parseProgress(jsonStr: String): ScanProgress {
        if (jsonStr.isEmpty() || jsonStr == "{}") return ScanProgress()
        return try {
            val obj = JSONObject(jsonStr)
            ScanProgress(
                totalBytes = obj.optLong("total_bytes", 0L),
                totalFiles = obj.optLong("total_files", 0L),
                activeWorkers = obj.optInt("active_workers", 0),
                elapsedMillis = obj.optLong("elapsed_millis", 0L),
                filesPerSec = obj.optDouble("files_per_sec", 0.0),
                bytesPerSec = obj.optDouble("bytes_per_sec", 0.0),
                isComplete = obj.optBoolean("is_complete", false),
                currentPath = obj.optString("current_path", "")
            )
        } catch (_: Exception) {
            ScanProgress()
        }
    }

    fun parseTreemapNodes(jsonStr: String): List<TreemapNode> {
        if (jsonStr.isEmpty() || jsonStr == "[]") return emptyList()
        val list = mutableListOf<TreemapNode>()
        try {
            val arr = JSONArray(jsonStr)
            for (i in 0 until arr.length()) {
                val obj = arr.getJSONObject(i)
                val cArr = obj.optJSONArray("children_ids")
                val children = mutableListOf<Int>()
                if (cArr != null) {
                    for (j in 0 until cArr.length()) {
                        children.add(cArr.getInt(j))
                    }
                }
                list.add(
                    TreemapNode(
                        id = obj.getInt("id"),
                        parentId = obj.getInt("parent_id"),
                        name = obj.getString("name"),
                        totalBytes = obj.getLong("total_bytes"),
                        directBytes = obj.getLong("direct_bytes"),
                        relDepth = obj.getInt("rel_depth"),
                        isDir = obj.getBoolean("is_dir"),
                        extension = obj.getString("extension"),
                        childrenIds = children
                    )
                )
            }
        } catch (_: Exception) {}
        return list
    }

    fun parseExtensionStats(jsonStr: String): List<ExtensionStat> {
        if (jsonStr.isEmpty() || jsonStr == "[]") return emptyList()
        val list = mutableListOf<ExtensionStat>()
        try {
            val arr = JSONArray(jsonStr)
            for (i in 0 until arr.length()) {
                val obj = arr.getJSONObject(i)
                list.add(
                    ExtensionStat(
                        extension = obj.getString("extension"),
                        totalBytes = obj.getLong("total_bytes"),
                        fileCount = obj.getLong("file_count"),
                        percentage = obj.getDouble("percentage").toFloat()
                    )
                )
            }
        } catch (_: Exception) {}
        return list
    }
}
