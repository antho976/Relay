package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.bounceClick
import java.io.File

/**
 * The identity hero — a tappable avatar, the editable name, and a "since · streak" line, all on one
 * card. This is the page's anchor: it makes the screen read as a profile rather than a list of links,
 * and finally surfaces the avatar the data layer already supports.
 */
@Composable
internal fun ProfileHeaderCard(
    name: String,
    sinceLabel: String,
    streakDays: Int,
    hasAvatar: Boolean,
    avatarFile: File,
    avatarStamp: Long,
    onSetName: (String) -> Unit,
    onPickAvatar: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color
) {
    var editing by remember { mutableStateOf(false) }
    var input by remember { mutableStateOf("") }

    ProfileCard(padding = 18.dp) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            AvatarCircle(hasAvatar, avatarFile, avatarStamp, name, accent, onPickAvatar)
            Spacer(Modifier.width(16.dp))
            Column(Modifier.weight(1f)) {
                if (editing) {
                    val focus = remember { FocusRequester() }
                    LaunchedEffect(Unit) { focus.requestFocus() }
                    // Commit on Done OR focus loss so a typed name is never silently lost; blank is
                    // ignored (it would otherwise wipe back to the "Athlete" placeholder).
                    fun commit() {
                        val trimmed = input.trim()
                        if (trimmed.isNotEmpty() && trimmed != name) onSetName(trimmed)
                        editing = false
                    }
                    BasicTextField(
                        value = input,
                        onValueChange = { input = it.take(30) },
                        singleLine = true,
                        textStyle = MaterialTheme.typography.headlineMedium.copy(color = onBg),
                        cursorBrush = SolidColor(accent),
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                        keyboardActions = KeyboardActions(onDone = { commit() }),
                        modifier = Modifier.fillMaxWidth().focusRequester(focus)
                            .onFocusChanged { if (!it.isFocused && editing) commit() }
                    )
                } else {
                    Text(
                        name.ifBlank { "Athlete" },
                        style = MaterialTheme.typography.headlineMedium,
                        color = onBg,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.bounceClick { input = name; editing = true }
                    )
                }
                Spacer(Modifier.height(8.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (sinceLabel.isNotBlank()) {
                        Text(
                            "SINCE ${sinceLabel.uppercase()}",
                            style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp
                        )
                        if (streakDays >= 2) Spacer(Modifier.width(10.dp))
                    }
                    if (streakDays >= 2) StreakChip(streakDays, accent)
                }
            }
        }
    }
}

/** Round avatar with a faint accent ring; the picked photo if there is one, else the name's initial. */
@Composable
private fun AvatarCircle(
    hasAvatar: Boolean,
    file: File,
    stamp: Long,
    name: String,
    accent: Color,
    onPick: () -> Unit
) {
    Box(
        Modifier.size(64.dp).clip(CircleShape).bounceClick { onPick() },
        contentAlignment = Alignment.Center
    ) {
        if (hasAvatar) {
            // Re-key on the file's stamp so a freshly-picked avatar reloads (the decoder caches by path).
            key(stamp) { ProgressPhotoImage(file, Modifier.size(64.dp).clip(CircleShape), reqPx = 256) }
        } else {
            Box(
                Modifier.size(64.dp).background(accent.copy(alpha = 0.15f)),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    name.trim().firstOrNull()?.uppercase() ?: "+",
                    style = MaterialTheme.typography.headlineSmall, color = accent
                )
            }
        }
        // Ring drawn last so it sits on top of the photo edge.
        Box(Modifier.size(64.dp).border(1.5.dp, accent.copy(alpha = 0.45f), CircleShape))
    }
}

/** A small accent pill calling out an active streak. */
@Composable
private fun StreakChip(days: Int, accent: Color) {
    Text(
        "$days-DAY STREAK",
        style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 9.sp,
        modifier = Modifier
            .clip(RoundedCornerShape(50))
            .background(accent.copy(alpha = 0.15f))
            .padding(horizontal = 8.dp, vertical = 3.dp)
    )
}
