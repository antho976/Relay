@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.recipes

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.theme.ForgeTheme

/**
 * RECIPE — Modal archetype (DESIGN §3).
 *
 * The ONE place the open-editorial rule inverts: a sheet KEEPS its surface. Everywhere else a fill
 * is earned by interactivity (§1); a modal is a surface by definition, because it has to lift off
 * the page it covers.
 *
 *   sheet   Surface fill + `large` top corners (16dp), content on 24dp gutter   §7
 *   ├─ one mono anchor naming what this sheet is about
 *   ├─ the transient detail of ONE tapped item                                  §4.4
 *   └─ actions: ① filled + ② outlined sidekick, at the END                      §8
 *
 * Sheet vs. routed sub-screen (§4.4): a sheet is the TRANSIENT detail of one tapped item. If it has
 * its own hero, its own lenses, or you would want to deep-link to it, it is a real sub-feature and
 * belongs on a route instead.
 *
 * Dialogs are for confirmations and tiny inputs ONLY — and per §12 a reversible act does not get a
 * dialog at all: it commits immediately and raises an Undo snackbar through `SnackbarController`.
 *
 * Hosting: wrap [ModalRecipeContent] in a `ModalBottomSheet(onDismissRequest = …, sheetState = …)`.
 * The content is separated out here so it previews on its own — the sheet chrome adds nothing you
 * need to design.
 */
@Composable
fun ModalRecipeContent(
    onConfirm: () -> Unit = {},
    onDismiss: () -> Unit = {},
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    Column(
        Modifier
            .fillMaxWidth()
            // §7: sheet top corners are 16dp — no custom radii. The surface fill is the exception
            // §1 grants to modals.
            .clip(RoundedCornerShape(topStart = 16.dp, topEnd = 16.dp))
            .background(MaterialTheme.colorScheme.surface)
            .padding(horizontal = 24.dp, vertical = 20.dp)
    ) {
        // §4.6 applies here too: the sheet does not repeat a title the row you tapped already said.
        EditorialHeader("THAT DAY", muted, accent)
        Spacer(Modifier.height(10.dp))

        Text("Pull B", style = MaterialTheme.typography.headlineSmall, color = onBg)
        Spacer(Modifier.height(2.dp))
        Text(
            "Monday 24 Jul · 48 min",
            style = MaterialTheme.typography.labelMedium,
            color = muted
        )

        Spacer(Modifier.height(16.dp))
        SheetRow("Sets", "12")
        SheetRow("Volume", "4,480 kg")
        SheetRow("Best e1RM", "88.7 kg")

        // §8: actions at the END, ① filled with its ② outlined sidekick. Never two filled capsules.
        Spacer(Modifier.height(24.dp))
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            ForgePrimaryCapsule("Open session", onConfirm, Modifier.weight(1f))
            ForgeOutlineCapsule("Close", onDismiss, Modifier.weight(1f))
        }
        Spacer(Modifier.height(8.dp))
    }
}

@Composable
private fun SheetRow(label: String, value: String) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        Modifier.fillMaxWidth().padding(vertical = 6.dp),   // §7: one padding for all rows here
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(
            label.uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            modifier = Modifier.weight(1f)
        )
        Text(value, style = MaterialTheme.typography.bodyMedium, color = onBg)
    }
}

@Preview(name = "Modal sheet", showBackground = true, backgroundColor = 0xFF0E0E11)
@Composable
private fun ModalRecipePreview() {
    ForgeTheme { ModalRecipeContent() }
}

@Preview(name = "Modal sheet · 200% font", showBackground = true, backgroundColor = 0xFF0E0E11, fontScale = 2.0f)
@Composable
private fun ModalRecipeLargeFontPreview() {
    ForgeTheme { ModalRecipeContent() }
}
