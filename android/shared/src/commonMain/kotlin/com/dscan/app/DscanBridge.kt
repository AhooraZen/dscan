package com.dscan.app

import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.InputStream

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

data class DriveInfo(
    val name: String,
    val mountPoint: String,
    val totalSpace: Long,
    val availableSpace: Long,
    val fileSystem: String
) {
    val usedSpace: Long get() = (totalSpace - availableSpace).coerceAtLeast(0L)
    val usedPercentage: Double get() = if (totalSpace > 0) (usedSpace.toDouble() / totalSpace.toDouble()) * 100.0 else 0.0
}

object DscanBridge {
    private var isLoaded = false

    init {
        loadNativeLibrary()
    }

    private fun loadNativeLibrary() {
        // 1. Try standard system library load (works on Android and if LD_LIBRARY_PATH is set)
        try {
            System.loadLibrary("dscan")
            isLoaded = true
            return
        } catch (_: UnsatisfiedLinkError) {}

        // 2. Try development build targets (target/release or target/debug relative to current working dir)
        val osName = System.getProperty("os.name")?.lowercase() ?: ""
        val libName = when {
            osName.contains("win") -> "dscan.dll"
            osName.contains("mac") -> "libdscan.dylib"
            else -> "libdscan.so"
        }

        val candidatePaths = listOf(
            libName,
            "target/release/$libName",
            "target/debug/$libName",
            "../target/release/$libName",
            "../target/debug/$libName",
            "../../target/release/$libName",
            "../../target/debug/$libName"
        )

        for (path in candidatePaths) {
            val file = File(path)
            if (file.exists() && file.isFile) {
                try {
                    System.load(file.absolutePath)
                    isLoaded = true
                    return
                } catch (_: Throwable) {}
            }
        }

        // 3. Fallback: extract bundled native library from JAR resources
        val arch = System.getProperty("os.arch")?.lowercase() ?: ""
        val resourcePrefix = when {
            osName.contains("win") -> if (arch.contains("64")) "win-x64" else "win-x86"
            osName.contains("mac") -> if (arch.contains("aarch64") || arch.contains("arm")) "mac-arm64" else "mac-x64"
            else -> if (arch.contains("aarch64") || arch.contains("arm")) "linux-arm64" else "linux-x64"
        }

        val resourcePaths = listOf(
            "/$libName",
            "/native/$resourcePrefix/$libName",
            "native/$resourcePrefix/$libName"
        )
        for (rPath in resourcePaths) {
            try {
                val stream: InputStream? = DscanBridge::class.java.getResourceAsStream(rPath)
                    ?: DscanBridge::class.java.classLoader?.getResourceAsStream(rPath.trimStart('/'))
                if (stream != null) {
                    val tempFile = File.createTempFile("libdscan-", "-${System.currentTimeMillis()}-$libName")
                    tempFile.deleteOnExit()
                    tempFile.outputStream().use { out ->
                        stream.copyTo(out)
                    }
                    System.load(tempFile.absolutePath)
                    isLoaded = true
                    return
                }
            } catch (_: Throwable) {}
        }

        if (!isLoaded) {
            System.err.println("DscanBridge: Warning - libdscan native library could not be loaded.")
        }
    }

    external fun startScan(path: String, threads: Int): Long
    external fun pollProgress(sessionPtr: Long): String
    external fun getTreemapNodes(sessionPtr: Long, maxDepth: Int, maxNodes: Int): String
    external fun getExtensionBreakdown(sessionPtr: Long, limit: Int): String
    external fun cancelScan(sessionPtr: Long)
    external fun stopScan(sessionPtr: Long)
    external fun detectDrives(): String
    external fun revealInFileManager(path: String): Boolean
    external fun moveToTrash(path: String, rootPath: String): String?

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

    fun parseDrives(jsonStr: String): List<DriveInfo> {
        if (jsonStr.isEmpty() || jsonStr == "[]") return emptyList()
        val list = mutableListOf<DriveInfo>()
        try {
            val arr = JSONArray(jsonStr)
            for (i in 0 until arr.length()) {
                val obj = arr.getJSONObject(i)
                list.add(
                    DriveInfo(
                        name = obj.getString("name"),
                        mountPoint = obj.getString("mount_point"),
                        totalSpace = obj.getLong("total_space"),
                        availableSpace = obj.getLong("available_space"),
                        fileSystem = obj.getString("file_system")
                    )
                )
            }
        } catch (_: Exception) {}
        return list
    }
}
