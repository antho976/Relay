package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.PhotoCamera
import androidx.compose.material.icons.outlined.SearchOff
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import com.forge.app.ui.common.ForgeSecondaryCapsule
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.data.repo.ProgressPhoto
import com.forge.app.domain.units.WeightUnit
import java.io.File
import java.time.ZoneId

/**
 * The gallery's scrolling body, emitted into the screen's lazy list: the compare shortcuts, then the
 * month-grouped grid. The grid's rows are the list's own items, so a large library only composes and
 * decodes what is near the screen.
 */
@Suppress("LongParameterList")
internal fun LazyListScope.galleryLibrary(
    loading: Boolean,
    libraryEmpty: Boolean,
    filter: GalleryFilter,
    sections: List<GallerySection>,
    suggestions: List<CompareSuggestion>,
    grid: GalleryGridSpec,
    zone: ZoneId,
    weightUnit: WeightUnit,
    fileFor: (ProgressPhoto) -> File,
    onCompare: (ProgressPhoto, ProgressPhoto) -> Unit,
    onTakePhoto: () -> Unit,
    onImport: () -> Unit,
    onClearFilters: () -> Unit
) {
    // Reads are local and fast, so there is no loading screen: the list stays empty for the few
    // milliseconds the index takes rather than flashing a placeholder the user cannot act on.
    if (loading) return

    if (libraryEmpty) {
        item(key = "empty") { EmptyLibrary(onTakePhoto, onImport) }
        return
    }

    if (sections.isEmpty()) {
        item(key = "no-results") {
            GalleryMessage(
                icon = Icons.Outlined.SearchOff,
                title = if (filter.searching) "Nothing matches “${filter.query.trim()}”" else "No photos match these filters",
                body = if (filter.album != null && filter.activeFacets == 1 && !filter.searching) {
                    "Select photos in the grid and choose Move to album to fill it."
                } else null
            ) {
                ForgeSecondaryCapsule(
                    label = if (filter.searching) "Clear search and filters" else "Clear filters",
                    onClick = onClearFilters
                )
            }
        }
        return
    }

    if (suggestions.isNotEmpty() && !grid.selecting) {
        item(key = "compare", contentType = "compare") {
            GalleryCompareStrip(suggestions, zone, weightUnit, fileFor, onCompare)
        }
    }

    galleryGrid(sections, grid)
}

/**
 * What an empty gallery says: what it is for, and the two ways in. The floating button stays away
 * until there is a grid, so the page never offers the same action twice.
 */
@Composable
private fun EmptyLibrary(onTakePhoto: () -> Unit, onImport: () -> Unit) {
    GalleryMessage(
        icon = Icons.Outlined.PhotoCamera,
        title = "No progress photos yet",
        body = "Shoot the same pose in the same spot every few weeks and compare them here. " +
            "Photos stay on this phone."
    ) {
        Button(onClick = onTakePhoto, modifier = Modifier.fillMaxWidth()) {
            Icon(Icons.Outlined.PhotoCamera, contentDescription = null, Modifier.size(ButtonDefaults.IconSize))
            Spacer(Modifier.size(ButtonDefaults.IconSpacing))
            Text("Take photo")
        }
        ForgeSecondaryCapsule(label = "Import from phone", onClick = onImport, modifier = Modifier.fillMaxWidth())
    }
}

/** A centred icon, title, optional body, and actions: the shape of every gallery state with no grid. */
@Composable
private fun GalleryMessage(
    icon: ImageVector,
    title: String,
    body: String?,
    actions: @Composable () -> Unit
) {
    Box(Modifier.fillMaxWidth().padding(horizontal = 32.dp, vertical = 56.dp), contentAlignment = Alignment.Center) {
        Column(
            Modifier.widthIn(max = 360.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Box(
                Modifier.size(72.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceContainerHigh),
                contentAlignment = Alignment.Center
            ) {
                Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(32.dp))
            }
            Spacer(Modifier.height(20.dp))
            Text(
                title,
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onBackground,
                textAlign = TextAlign.Center
            )
            if (body != null) {
                Spacer(Modifier.height(8.dp))
                Text(
                    body,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center
                )
            }
            Spacer(Modifier.height(24.dp))
            Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                actions()
            }
        }
    }
}
