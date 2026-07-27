package com.forge.app.ui.checkin

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.GlyphButton
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.ForgeMotion

/**
 * The morning check-in's front door: a top-anchored invitation rather than a sheet that opens itself.
 *
 * The check-in used to arrive as a [androidx.compose.material3.ModalBottomSheet] at the day's first
 * launch — over whatever the app was actually opened for, with no way past it but to answer or
 * dismiss. Same question, asked from the top: the app underneath stays usable, and the sheet opens
 * only when it is chosen.
 *
 * It is neither an error banner (§12 keeps errors as quiet inline lines) nor a coach line (§11 keeps
 * those as italic asides) — it is a transient prompt, the top-anchored sibling of the app's one Undo
 * snackbar, and it earns its surface by being tappable (§1). Two ways out, both one tap: the plate
 * opens the sheet, the `×` skips the day.
 */
@Composable
fun CheckinPromptBanner(
    visible: Boolean,
    onOpen: () -> Unit,
    onDismiss: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    // A full-size, unpainted Box only anchors the plate to the top; it draws nothing and holds no
    // pointer input, so taps outside the banner reach the screen beneath (as SnackbarControllerHost).
    Box(
        Modifier.fillMaxSize().statusBarsPadding(),
        contentAlignment = Alignment.TopCenter
    ) {
        AnimatedVisibility(
            visible = visible,
            enter = slideInVertically(ForgeMotion.enterTween<IntOffset>()) { -it } +
                fadeIn(ForgeMotion.enterTween()),
            exit = slideOutVertically(ForgeMotion.exitTween<IntOffset>()) { -it } +
                fadeOut(ForgeMotion.exitTween())
        ) {
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 12.dp, vertical = 8.dp)
                    .clip(MaterialTheme.shapes.medium)
                    .background(MaterialTheme.colorScheme.surface)
                    .bounceClick(onClickLabel = "Open this morning's check-in") { onOpen() }
                    // end padding is small because the dismiss glyph carries its own 48dp target.
                    .padding(start = 16.dp, top = 12.dp, bottom = 12.dp, end = 4.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(Modifier.weight(1f)) {
                    Text(
                        "THIS MORNING",
                        style = MaterialTheme.typography.labelMedium,
                        color = muted,
                        letterSpacing = 1.sp
                    )
                    Spacer(Modifier.height(8.dp))
                    Text(
                        "Rate sleep, soreness, stress and drive",
                        style = MaterialTheme.typography.bodyMedium,
                        color = onBg
                    )
                }
                Spacer(Modifier.width(8.dp))
                GlyphButton(
                    glyph = "×",
                    label = "Skip today's check-in",
                    tint = muted,
                    onClick = onDismiss
                )
            }
        }
    }
}
