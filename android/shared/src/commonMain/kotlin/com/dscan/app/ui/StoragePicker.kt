package com.dscan.app.ui

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import java.io.File

data class StorageTarget(val label: String, val path: String)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun StoragePicker(
    selectedPath: String,
    onPathSelected: (String) -> Unit,
    targets: List<StorageTarget> = getDefaultStorageTargets(),
    modifier: Modifier = Modifier
) {
    if (targets.isEmpty()) return

    Row(
        modifier = modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        targets.forEach { target ->
            val isSelected = selectedPath == target.path
            FilterChip(
                selected = isSelected,
                onClick = { onPathSelected(target.path) },
                label = { Text(target.label) },
                colors = FilterChipDefaults.filterChipColors(
                    selectedContainerColor = MaterialTheme.colorScheme.primaryContainer,
                    selectedLabelColor = MaterialTheme.colorScheme.onPrimaryContainer
                )
            )
        }
    }
}

fun getDefaultStorageTargets(): List<StorageTarget> {
    val targets = mutableListOf<StorageTarget>()
    val androidStorage = File("/storage/emulated/0")
    if (androidStorage.exists()) {
        targets.add(StorageTarget("Internal Storage", androidStorage.absolutePath))
        val dl = File("/storage/emulated/0/Download")
        if (dl.exists()) targets.add(StorageTarget("Downloads", dl.absolutePath))
        val dcim = File("/storage/emulated/0/DCIM")
        if (dcim.exists()) targets.add(StorageTarget("Photos", dcim.absolutePath))
        val docs = File("/storage/emulated/0/Documents")
        if (docs.exists()) targets.add(StorageTarget("Documents", docs.absolutePath))
        return targets
    }

    val home = System.getenv("HOME") ?: System.getenv("USERPROFILE") ?: "."
    val homeFile = File(home)
    targets.add(StorageTarget("Home", homeFile.absolutePath))

    val downloads = File(home, "Downloads")
    if (downloads.exists()) targets.add(StorageTarget("Downloads", downloads.absolutePath))

    val docs = File(home, "Documents")
    if (docs.exists()) targets.add(StorageTarget("Documents", docs.absolutePath))

    return targets
}
