package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
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
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.bounceClick
import java.io.File

/** Height of the full-bleed identity banner spanning the top of the profile. */
private val BannerHeight = 240.dp

/**
 * Vertical alpha mask (used via [BlendMode.DstIn]) that fades the cover to transparent at the top and
 * bottom edges, so the photo dissolves into the app background instead of ending on a hard rectangle.
 *
 * The bottom half is an EASED ramp, not a straight line (2026-07-24, Antho): a linear alpha slope
 * hands most of its falloff to the last few dp and lands as a visible edge, so the tail is stretched
 * — 62% → 78% does the bulk, then a long thin toe carries the last ~10% of the photo out to nothing.
 * The dissolve is now the ONLY thing between cover and page; there is no second scrim on top of it.
 */
private val EdgeFade = Brush.verticalGradient(
    0f to Color.Transparent,
    0.14f to Color.Black,
    0.50f to Color.Black,
    0.62f to Color.Black.copy(alpha = 0.72f),
    0.72f to Color.Black.copy(alpha = 0.40f),
    0.82f to Color.Black.copy(alpha = 0.18f),
    0.91f to Color.Black.copy(alpha = 0.06f),
    1f to Color.Transparent
)

/** Soft dark halo behind the big name — legible over any photo, even where the scrim is thin. */
private val NameShadow = Shadow(color = Color.Black.copy(alpha = 0.65f), offset = Offset(0f, 2f), blurRadius = 14f)

/** Tighter halo for the small meta line (SINCE · streak), which washes out easiest over bright shots. */
private val MetaShadow = Shadow(color = Color.Black.copy(alpha = 0.75f), offset = Offset(0f, 1f), blurRadius = 6f)

/**
 * The identity hero — the profile picture rendered as a full-bleed COVER photo across the top of the
 * page (not a small round avatar), with the editable name and a "since" line laid over a bottom
 * scrim. Tapping the banner picks a new photo; tapping the name edits it. When no photo is set, a
 * quiet "tap to add" placeholder fills the banner instead.
 *
 * ## The streaks left the cover (2026-08-24)
 *
 * A "BEST n-DAY STREAK" caption and an accent streak chip used to sit above the name. Two things
 * were wrong with that and only one of them was fixable here.
 *
 * The chip was accent-on-photograph. Every other tone on this cover is white with a shadow halo
 * behind it, because that is the only treatment that survives an arbitrary image underneath; ember
 * at 0.18 alpha on a dark forest is a pill you have to hunt for, which is what Antho saw. And when
 * the current streak IS the best one — the common case, and the case a new user is always in — the
 * two elements printed the same number twice, a caption and a chip side by side both reading
 * "2-DAY STREAK".
 *
 * A streak is not identity, it is attendance, so it moved to the section that answers attendance:
 * [ProfileActivityMonth]'s readings line, beside ACTIVE DAYS and SESSIONS. It is legible there, it
 * sits next to the grid that shows the same days, and the best only prints when it beats the run
 * you are on. The cover keeps what a cover is for.
 */
@Composable
internal fun ProfileHeaderCard(
    name: String,
    sinceLabel: String,
    hasAvatar: Boolean,
    avatarFile: File,
    avatarStamp: Long,
    onSetName: (String) -> Unit,
    onPickAvatar: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    // Status-bar + app-bar inset. The banner grows UPWARD by this so the photo fills the whole top of
    // the screen (behind the status bar) instead of stopping below the bars; the name stays at the
    // bottom, so nothing below the banner shifts.
    topInset: Dp = 0.dp
) {
    var editing by remember { mutableStateOf(false) }
    var input by remember { mutableStateOf("") }

    Box(
        Modifier
            .fillMaxWidth()
            .height(BannerHeight + topInset)
            .clickable(onClickLabel = "Change profile photo") { onPickAvatar() }
    ) {
        // Media + darkening, alpha-faded into the app background at the top AND bottom edges so the
        // cover melts into the page instead of ending on a hard rectangular cut. The fade is an
        // offscreen DstIn mask on this layer only, so the name/meta drawn above it stay fully crisp.
        Box(
            Modifier
                .fillMaxSize()
                .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
                .drawWithContent {
                    drawContent()
                    drawRect(brush = EdgeFade, blendMode = BlendMode.DstIn)
                }
        ) {
            // Background: the profile photo cropped to fill, or a quiet "add a photo" placeholder.
            if (hasAvatar) {
                // Re-key on the file's stamp so a freshly-picked photo reloads (the decoder caches by path).
                // Match AvatarRepository.MAX_PX so the stored cover is shown at full resolution: a lower
                // cap here would pre-shrink a portrait's longest edge and drop its width below the screen
                // (→ upscaled + blurry). The decoder still bounds memory by scaling to exactly this.
                key(avatarStamp) { ProgressPhotoImage(avatarFile, Modifier.fillMaxSize(), reqPx = 2560) }
            } else {
                Box(
                    Modifier.fillMaxSize().background(
                        Brush.verticalGradient(listOf(accent.copy(alpha = 0.12f), accent.copy(alpha = 0.04f)))
                    ),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        // Fallback only — a default cover is normally seeded so this rarely shows; the tap
                        // now opens the chooser (own photo + provided covers), not the system picker (GYMAP-22).
                        "TAP TO CHOOSE A PHOTO",
                        style = MaterialTheme.typography.labelSmall,
                        color = muted.copy(alpha = 0.7f), fontSize = 10.sp, letterSpacing = 2.sp
                    )
                }
            }
        }

        // No second scrim. The 2026-07-09 version peaked at 0.85 black under the name and then
        // released back to transparent — which painted a band DARKER than the page background and
        // then lifted off it, so the "seam hider" was itself the seam Antho could see (2026-07-24).
        // The eased [EdgeFade] tail alone carries the photo to nothing, and it can only ever
        // approach the page background, never overshoot past it. The name and meta keep their own
        // [NameShadow] / [MetaShadow] haloes for legibility over a bright cover.

        // Identity laid over the bottom-left of the cover.
        Column(
            // 24dp = the page gutter (§7), so the name starts on the same rail as every section below.
            Modifier.align(Alignment.BottomStart).fillMaxWidth().padding(horizontal = 24.dp, vertical = 16.dp)
        ) {
            // The streak pair moved OFF the cover (Antho, 2026-08-24) — see the note on this file's
            // header. What is left over the photo is identity alone: the name, and when you started.
            // Name — tap to edit. Its own click consumes the tap so it doesn't also open the photo picker.
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
                    textStyle = MaterialTheme.typography.displaySmall.copy(color = Color.White, shadow = NameShadow),
                    cursorBrush = SolidColor(accent),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { commit() }),
                    modifier = Modifier.fillMaxWidth().focusRequester(focus)
                        .onFocusChanged { if (!it.isFocused && editing) commit() }
                )
            } else {
                Text(
                    name.ifBlank { "Athlete" },
                    style = MaterialTheme.typography.displaySmall.copy(shadow = NameShadow),
                    color = Color.White,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.bounceClick { input = name; editing = true }
                )
            }
            // "Since {month year}" — when you started (your first session), sitting under the name (GYMAP-23).
            if (sinceLabel.isNotBlank()) {
                Spacer(Modifier.height(4.dp))
                Text(
                    "SINCE ${sinceLabel.uppercase()}",
                    style = MaterialTheme.typography.labelSmall.copy(shadow = MetaShadow),
                    color = Color.White.copy(alpha = 0.9f),
                    fontSize = 9.sp, letterSpacing = 1.sp, maxLines = 1
                )
            }
        }
    }
}

