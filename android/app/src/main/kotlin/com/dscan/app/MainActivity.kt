package com.dscan.app

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.lifecycleScope
import com.dscan.app.ui.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MainActivity : ComponentActivity() {

    private var sessionPtr by mutableLongStateOf(0L)
    private var isScanning by mutableStateOf(false)
    private var scanProgress by mutableStateOf(ScanProgress())
    private var treemapNodes by mutableStateOf<List<TreemapNode>>(emptyList())
    private var extensionStats by mutableStateOf<List<ExtensionStat>>(emptyList())
    private var targetPath by mutableStateOf("")
    private var hasStoragePermission by mutableStateOf(false)
    private var showSettings by mutableStateOf(false)
    private var settings by mutableStateOf(ScanSettings())

    private val folderPicker = registerForActivityResult(
        ActivityResultContracts.OpenDocumentTree()
    ) { uri: Uri? ->
        uri?.let { treeUriToPath(it) }?.let { targetPath = it }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        targetPath = Environment.getExternalStorageDirectory().absolutePath
        checkStoragePermission()

        setContent {
            val darkTheme = when (settings.themeMode) {
                ThemeMode.Dark -> true
                ThemeMode.Light -> false
                ThemeMode.System -> androidx.compose.foundation.isSystemInDarkTheme()
            }
            DscanTheme(darkTheme = darkTheme) {
                if (!hasStoragePermission) {
                    PermissionRequestScreen(
                        onRequestPermission = { requestStoragePermission() }
                    )
                } else if (showSettings) {
                    SettingsScreen(
                        settings = settings,
                        onSettingsChange = { settings = it },
                        onBack = { showSettings = false }
                    )
                } else {
                    MainScreen(
                        currentPath = targetPath,
                        onPathChange = { targetPath = it },
                        progress = scanProgress,
                        isScanning = isScanning,
                        treemapNodes = treemapNodes,
                        extensionStats = extensionStats,
                        onStartScan = { startScan(targetPath) },
                        onCancelScan = { cancelScan() },
                        onBrowseFolder = { folderPicker.launch(null) },
                        onNavigateSettings = { showSettings = true }
                    )
                }
            }
        }
    }

    override fun onResume() {
        super.onResume()
        checkStoragePermission()
    }

    override fun onDestroy() {
        super.onDestroy()
        if (sessionPtr != 0L) {
            DscanBridge.stopScan(sessionPtr)
            sessionPtr = 0L
        }
    }

    private fun checkStoragePermission() {
        hasStoragePermission = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Environment.isExternalStorageManager()
        } else {
            true
        }
    }

    private fun requestStoragePermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            try {
                val intent = Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
                    data = Uri.parse("package:$packageName")
                }
                startActivity(intent)
            } catch (_: Exception) {
                val intent = Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION)
                startActivity(intent)
            }
        }
    }

    private fun treeUriToPath(uri: Uri): String? {
        // SAF tree URI → filesystem path
        // Format: content://com.android.externalstorage.documents/tree/primary:path
        val docId = try {
            // DocumentsContract.getTreeDocumentId requires API 21+
            val treeDocId = uri.pathSegments
            if (treeDocId.size >= 2) treeDocId[1] else return null
        } catch (_: Exception) {
            return null
        }
        val split = docId.split(":")
        return if (split[0].equals("primary", ignoreCase = true)) {
            val base = Environment.getExternalStorageDirectory().absolutePath
            if (split.size > 1 && split[1].isNotEmpty()) "$base/${split[1]}" else base
        } else {
            // External SD or other volume
            "/storage/${split[0]}" + if (split.size > 1 && split[1].isNotEmpty()) "/${split[1]}" else ""
        }
    }

    private fun startScan(path: String) {
        if (isScanning) return
        isScanning = true
        scanProgress = ScanProgress()
        treemapNodes = emptyList()
        extensionStats = emptyList()

        val threads = settings.threadCount
        val maxDepth = settings.maxTreemapDepth
        val maxNodes = settings.maxTreemapNodes

        lifecycleScope.launch(Dispatchers.IO) {
            if (sessionPtr != 0L) {
                DscanBridge.stopScan(sessionPtr)
                sessionPtr = 0L
            }

            val ptr = DscanBridge.startScan(path, threads)
            if (ptr == 0L) {
                withContext(Dispatchers.Main) { isScanning = false }
                return@launch
            }
            withContext(Dispatchers.Main) { sessionPtr = ptr }

            while (isActive && isScanning) {
                val progressJson = DscanBridge.pollProgress(ptr)
                val prog = DscanBridge.parseProgress(progressJson)
                withContext(Dispatchers.Main) {
                    scanProgress = prog
                }

                if (prog.isComplete) {
                    val nodesJson = DscanBridge.getTreemapNodes(ptr, maxDepth, maxNodes)
                    val statsJson = DscanBridge.getExtensionBreakdown(ptr, 30)
                    val parsedNodes = DscanBridge.parseTreemapNodes(nodesJson)
                    val parsedStats = DscanBridge.parseExtensionStats(statsJson)

                    withContext(Dispatchers.Main) {
                        treemapNodes = parsedNodes
                        extensionStats = parsedStats
                        isScanning = false
                    }
                    break
                }
                delay(100)
            }
        }
    }

    private fun cancelScan() {
        if (sessionPtr != 0L) {
            DscanBridge.cancelScan(sessionPtr)
        }
        isScanning = false
    }
}

@Composable
fun PermissionRequestScreen(
    onRequestPermission: () -> Unit
) {
    Surface(
        modifier = Modifier.fillMaxSize(),
        color = MaterialTheme.colorScheme.background
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(32.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            Text(
                text = "All Files Access Required",
                style = MaterialTheme.typography.headlineSmall,
                color = MaterialTheme.colorScheme.primary
            )
            Spacer(modifier = Modifier.height(16.dp))
            Text(
                text = "dscan is a high-speed disk space analyzer. To scan external storage and visualize file usage, it requires full storage access.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(modifier = Modifier.height(24.dp))
            Button(
                onClick = onRequestPermission,
                colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary)
            ) {
                Text("Grant Storage Access")
            }
        }
    }
}
