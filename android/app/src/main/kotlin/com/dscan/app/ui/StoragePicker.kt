package com.dscan.app.ui

import android.os.Environment
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

data class StorageTarget(val label: String, val path: String)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun StoragePicker(
    selectedPath: String,
    onPathSelected: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    val defaultStorage = Environment.getExternalStorageDirectory().absolutePath
    val targets = listOf(
        StorageTarget("Internal Storage", defaultStorage),
        StorageTarget("Downloads", "$defaultStorage/Download"),
        StorageTarget("DCIM / Photos", "$defaultStorage/DCIM"),
        StorageTarget("Documents", "$defaultStorage/Documents"),
        StorageTarget("Movies", "$defaultStorage/Movies"),
        StorageTarget("Music", "$defaultStorage/Music")
    )

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
