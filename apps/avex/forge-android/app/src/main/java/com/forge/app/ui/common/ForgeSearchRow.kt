package com.forge.app.ui.common

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

/**
 * The app's one search field: a filled, fully rounded row with a leading magnifier, the query typed
 * inline, and a small clear capsule once there is something to clear. No outline; the raised fill
 * is the affordance. The row holds 52dp whether or not the capsule shows, so typing the first
 * character never moves the list below it.
 *
 * Goals and the gym screens each kept their own copy (`ForgeSearchRow`, `ForgeSearchRow`) that had
 * drifted 2dp apart in inset and spacing and used different clear buttons.
 */
@Composable
internal fun ForgeSearchRow(
    query: String,
    onQueryChange: (String) -> Unit,
    placeholder: String,
    modifier: Modifier = Modifier
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = modifier
            .fillMaxWidth()
            .heightIn(min = 52.dp)
            .clip(RoundedCornerShape(GROUP_OUTER))
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .padding(start = ROW_H, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Icon(Icons.Filled.Search, contentDescription = null, tint = muted, modifier = Modifier.size(20.dp))
        BasicTextField(
            value = query,
            onValueChange = onQueryChange,
            textStyle = MaterialTheme.typography.bodyLarge.copy(color = onBg),
            cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
            singleLine = true,
            modifier = Modifier
                .weight(1f)
                .padding(vertical = 14.dp)
                .semantics { contentDescription = placeholder },
            decorationBox = { inner ->
                Box {
                    if (query.isEmpty()) {
                        // A placeholder is the one text allowed under the muted floor (§5).
                        Text(placeholder, style = MaterialTheme.typography.bodyLarge, color = muted.copy(alpha = 0.6f))
                    }
                    inner()
                }
            }
        )
        if (query.isNotEmpty()) {
            Box(
                Modifier
                    .minimumInteractiveComponentSize()
                    .clip(RoundedCornerShape(50))
                    .clickableLabeled("Clear search") { onQueryChange("") },
                contentAlignment = Alignment.Center
            ) {
                Box(
                    Modifier
                        .size(28.dp)
                        .clip(RoundedCornerShape(50))
                        .background(MaterialTheme.colorScheme.surfaceContainerHighest),
                    contentAlignment = Alignment.Center
                ) {
                    Icon(Icons.Filled.Close, contentDescription = null, tint = muted, modifier = Modifier.size(16.dp))
                }
            }
        } else {
            Box(Modifier.size(12.dp))
        }
    }
}
