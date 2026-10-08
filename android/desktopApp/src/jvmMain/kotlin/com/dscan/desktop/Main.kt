package com.dscan.desktop

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.*
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.*
import com.dscan.app.*
import com.dscan.app.ui.*
import kotlinx.coroutines.*
import java.awt.Dimension
import java.awt.datatransfer.DataFlavor
import java.awt.dnd.DnDConstants
import java.awt.dnd.DropTarget
import java.awt.dnd.DropTargetDropEvent
import java.io.File
import javax.swing.JFileChooser
import javax.swing.JOptionPane

fun main() = application {
    val windowState = rememberWindowState(
        size = DpSize(1280.dp, 800.dp),
        position = WindowPosition.PlatformDefault
    )

    var targetPath by remember { mutableStateOf(System.getProperty("user.home") ?: ".") }
    var sessionPtr by remember { mutableStateOf(0L) }
    var isScanning by remember { mutableStateOf(false) }
    var scanProgress by remember { mutableStateOf(ScanProgress()) }
    var treemapNodes by remember { mutableStateOf<List<TreemapNode>>(emptyList()) }
    var extensionStats by remember { mutableStateOf<List<ExtensionStat>>(emptyList()) }
    var drives by remember { mutableStateOf<List<DriveInfo>>(emptyList()) }

    var searchQuery by remember { mutableStateOf("") }
    var showSettings by remember { mutableStateOf(false) }
    var settings by remember { mutableStateOf(ScanSettings()) }
    val systemDark = isSystemInDarkTheme()
    var isDarkTheme by remember {
        mutableStateOf(
            when (settings.themeMode) {
                ThemeMode.Dark -> true
                ThemeMode.Light -> false
                ThemeMode.System -> systemDark
            }
        )
    }

    val coroutineScope = rememberCoroutineScope()

    // Detect drives on startup
    LaunchedEffect(Unit) {
        withContext(Dispatchers.IO) {
            try {
                val json = DscanBridge.detectDrives()
                drives = DscanBridge.parseDrives(json)
            } catch (_: Throwable) {}
        }
    }

    fun startScanFor(path: String) {
        if (isScanning) return
        isScanning = true
        scanProgress = ScanProgress()
        treemapNodes = emptyList()
        extensionStats = emptyList()

        val threads = settings.threadCount
        val maxDepth = settings.maxTreemapDepth
        val maxNodes = settings.maxTreemapNodes

        coroutineScope.launch(Dispatchers.IO) {
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
                delay(80)
            }
        }
    }

    fun cancelScan() {
        if (sessionPtr != 0L) {
            DscanBridge.cancelScan(sessionPtr)
        }
        isScanning = false
    }

    fun pickFolder() {
        val chooser = JFileChooser(targetPath).apply {
            fileSelectionMode = JFileChooser.DIRECTORIES_ONLY
            dialogTitle = "Select Directory to Scan"
        }
        val result = chooser.showOpenDialog(null)
        if (result == JFileChooser.APPROVE_OPTION && chooser.selectedFile != null) {
            val selected = chooser.selectedFile.absolutePath
            targetPath = selected
            startScanFor(selected)
        }
    }

    Window(
        onCloseRequest = {
            if (sessionPtr != 0L) {
                DscanBridge.stopScan(sessionPtr)
            }
            exitApplication()
        },
        title = "dscan — Disk Space Visualizer",
        state = windowState,
        onKeyEvent = { event ->
            if (event.type == KeyEventType.KeyDown) {
                if (event.isCtrlPressed && event.key == Key.O) {
                    pickFolder()
                    true
                } else if (event.isCtrlPressed && event.key == Key.R) {
                    startScanFor(targetPath)
                    true
                } else if (event.isCtrlPressed && event.key == Key.T) {
                    isDarkTheme = !isDarkTheme
                    true
                } else if (event.isCtrlPressed && event.key == Key.Q) {
                    exitApplication()
                    true
                } else false
            } else false
        }
    ) {
        // Window setup: minimum size and Drag & Drop handler
        LaunchedEffect(window) {
            window.minimumSize = Dimension(960, 640)

            window.dropTarget = object : DropTarget() {
                override fun drop(evt: DropTargetDropEvent) {
                    try {
                        evt.acceptDrop(DnDConstants.ACTION_COPY)
                        val droppedFiles = evt.transferable.getTransferData(DataFlavor.javaFileListFlavor) as? List<*>
                        val firstFile = droppedFiles?.firstOrNull() as? File
                        if (firstFile != null && firstFile.isDirectory) {
                            targetPath = firstFile.absolutePath
                            startScanFor(targetPath)
                        }
                    } catch (_: Exception) {}
                }
            }
        }

        // Desktop Menu Bar
        MenuBar {
            Menu("File", mnemonic = 'F') {
                Item("Open Directory...", onClick = { pickFolder() }, shortcut = KeyShortcut(Key.O, ctrl = true))
                Item("Rescan", onClick = { startScanFor(targetPath) }, shortcut = KeyShortcut(Key.R, ctrl = true))
                Separator()
                Item("Exit", onClick = { exitApplication() }, shortcut = KeyShortcut(Key.Q, ctrl = true))
            }
            Menu("View", mnemonic = 'V') {
                Item("Toggle Theme", onClick = { isDarkTheme = !isDarkTheme }, shortcut = KeyShortcut(Key.T, ctrl = true))
                Separator()
                Item("Settings", onClick = { showSettings = true })
            }
            Menu("Help", mnemonic = 'H') {
                Item("About dscan", onClick = {
                    JOptionPane.showMessageDialog(
                        window,
                        "dscan v0.7.0\nHigh-Performance Disk Space Analyzer\nZero external dependencies • Linux kernel & Win32 native traversal\nPowered by Compose Multiplatform & Parch Linux Design Tokens.",
                        "About dscan",
                        JOptionPane.INFORMATION_MESSAGE
                    )
                })
            }
        }

        DscanTheme(darkTheme = isDarkTheme) {
            Box(modifier = Modifier.fillMaxSize()) {
                if (showSettings) {
                    SettingsScreen(
                        settings = settings,
                        onSettingsChange = {
                            settings = it
                            isDarkTheme = when (it.themeMode) {
                                ThemeMode.Dark -> true
                                ThemeMode.Light -> false
                                ThemeMode.System -> systemDark
                            }
                        },
                        onBack = { showSettings = false }
                    )
                } else {
                    DesktopMainView(
                        targetPath = targetPath,
                        onPathChange = { targetPath = it },
                        progress = scanProgress,
                        isScanning = isScanning,
                        treemapNodes = treemapNodes,
                        extensionStats = extensionStats,
                        drives = drives,
                        isDarkTheme = isDarkTheme,
                        onToggleTheme = { isDarkTheme = !isDarkTheme },
                        onStartScan = { startScanFor(targetPath) },
                        onCancelScan = { cancelScan() },
                        onBrowseFolder = { pickFolder() },
                        onOpenSettings = { showSettings = true },
                        searchQuery = searchQuery,
                        onSearchQueryChange = { searchQuery = it }
                    )
                }
            }
        }
    }
}
