package com.dscan.app

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.lifecycleScope
import com.dscan.app.ui.DscanTheme
import com.dscan.app.ui.MainScreen
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
    private var isDarkTheme by mutableStateOf(true)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        targetPath = Environment.getExternalStorageDirectory().absolutePath
        checkStoragePermission()

        setContent {
            DscanTheme(darkTheme = isDarkTheme) {
                if (!hasStoragePermission) {
                    PermissionRequestScreen(
                        onRequestPermission = { requestStoragePermission() }
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
                        darkTheme = isDarkTheme,
                        onToggleDarkTheme = { isDarkTheme = !isDarkTheme }
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

    private fun startScan(path: String) {
        if (isScanning) return
        isScanning = true
        scanProgress = ScanProgress()
        treemapNodes = emptyList()
        extensionStats = emptyList()

        lifecycleScope.launch(Dispatchers.IO) {
            if (sessionPtr != 0L) {
                DscanBridge.stopScan(sessionPtr)
                sessionPtr = 0L
            }

            // 0 auto-detects CPU thread count
            val ptr = DscanBridge.startScan(path, 0)
            if (ptr == 0L) {
                withContext(Dispatchers.Main) { isScanning = false }
                return@launch
            }
            withContext(Dispatchers.Main) { sessionPtr = ptr }

            // Polling loop
            while (isActive && isScanning) {
                val progressJson = DscanBridge.pollProgress(ptr)
                val prog = DscanBridge.parseProgress(progressJson)
                withContext(Dispatchers.Main) {
                    scanProgress = prog
                }

                if (prog.isComplete) {
                    val nodesJson = DscanBridge.getTreemapNodes(ptr, 6, 2000)
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
