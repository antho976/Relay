package com.forge.app.ui.profile

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.bounceClick
import java.io.File

/**
 * The profile's circular avatar. Shows the picked image (decoded app-private via [ProgressPhotoImage])
 * when set, otherwise the user's initial as a placeholder. Tapping it opens the Photo Picker. [stamp]
 * changes whenever the avatar is replaced, so the decoded bitmap is re-fetched even though the file
 * path (`avatar.jpg`) stays the same.
 */
@Composable
internal fun ProfileAvatar(
    hasAvatar: Boolean,
    file: File,
    stamp: Long,
    initial: String,
    muted: Color,
    outline: Color,
    onPick: () -> Unit
) {
    Box(
        Modifier
            .size(64.dp)
            .clip(CircleShape)
            .border(1.dp, outline.copy(alpha = 0.5f), CircleShape)
            .bounceClick { onPick() },
        contentAlignment = Alignment.Center
    ) {
        if (hasAvatar) {
            key(stamp) {
                ProgressPhotoImage(file, Modifier.fillMaxSize().clip(CircleShape), reqPx = 200)
            }
        } else {
            Text(
                initial.ifBlank { "+" },
                style = MaterialTheme.typography.headlineSmall,
                color = muted
            )
        }
    }
}
