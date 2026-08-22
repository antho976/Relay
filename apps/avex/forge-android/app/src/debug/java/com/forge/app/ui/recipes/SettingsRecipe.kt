@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.recipes

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.forge.app.ui.settings.ConnectPill
import com.forge.app.ui.settings.SettingsActionLink
import com.forge.app.ui.settings.SettingsActionRow
import com.forge.app.ui.settings.SettingsExplainer
import com.forge.app.ui.settings.SettingsOutlineAction
import com.forge.app.ui.settings.SettingsPrimaryAction
import com.forge.app.ui.settings.SettingsSectionHeader
import com.forge.app.ui.settings.SETTINGS_GUTTER
import com.forge.app.ui.settings.SETTINGS_ROW_PAD
import com.forge.app.ui.settings.SettingsNavRow
import com.forge.app.ui.settings.StatusDot
import com.forge.app.ui.settings.ToggleRow
import com.forge.app.ui.theme.ForgeTheme

/**
 * RECIPE — Settings / form / editor archetype (DESIGN §3).
 *
 * The archetype most often got wrong, because the editorial kit is tempting and all of it is banned
 * here. NO serif hero. NO figures. NO lens pills. NO chart motion or entrance cascade. NO dividers.
 *
 *   top bar (← + ≤1 action; the bell is Home only)
 *   ├─ SettingsSectionHeader  mono anchor + air, never a divider          §3, §7
 *   ├─ rows                   each control gets a ≤1-line explainer       §3
 *   │                         nav rows show their LIVE value              §3
 *   │                         WHOLE ROW is the ≥48dp tap target           §14, §2③
 *   ├─ per-row action         whole-row tap + drawn OUTLINED pill         §2③, §8
 *   └─ SettingsActionRow      filled ① + outlined ② GROUPED AT THE END    §8
 *
 * Two gutter rules, and they are the thing this recipe most exists to pin down:
 *  - `SettingsPrimitives` ROWS apply their own `SETTINGS_GUTTER` padding, so the page Column must
 *    NOT add it again.
 *  - page ACTIONS are gutterless capsules that only ever go inside [SettingsActionRow], which owns
 *    the gutter and wraps them at large font scales. Wrapping them in your own padded Row or a
 *    ChipFlow double-gutters them to 48dp — every shipped call site had that bug once.
 */
@Composable
fun SettingsRecipe(
    onBack: () -> Unit = {},
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant

    var keepAwake by remember { mutableStateOf(true) }
    var privacy by remember { mutableStateOf(false) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {},     // §4.6 — never "Settings" here
                navigationIcon = {
                    // §4.6 — ONE back affordance per page. No second in-page back arrow.
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Column(
            Modifier
                .padding(inner)
                .verticalScroll(rememberScrollState())
            // no horizontal padding — the row primitives own the 24dp gutter
        ) {
            // ── A section ───────────────────────────────────────────────────────────────────────
            // §3/§7: a mono anchor plus air IS the separator. Reaching for a divider here is the
            // "hairline habit" in FAILURES.md.
            SettingsSectionHeader("APPEARANCE")

            // §3: every control carries a ≤1-line explainer. It sits beside the control, never as
            // a paragraph above the section (§4.3 — mechanics narration is cut, not trimmed).
            ToggleRow(
                label = "Privacy mode",
                subtitle = "Hides the app in recents and blocks screenshots",
                checked = privacy,
                onCheckedChange = { privacy = it }
            )
            ToggleRow(
                label = "Keep screen on",
                subtitle = "Holds the display awake while you log",
                checked = keepAwake,
                onCheckedChange = { keepAwake = it }
            )

            // §3: a nav row shows its LIVE value in the subtitle, so the page answers without a tap.
            SettingsNavRow(
                label = "App icon",
                subtitle = "Metal",
                onClick = { }
            )

            // ── A list of connectables ──────────────────────────────────────────────────────────
            SettingsSectionHeader("RECOVERY")
            // §8: a per-row action is a DRAWN outlined pill with the WHOLE row as the tap target.
            // Five filled capsules stacked here is the canonical "button wall" (FAILURES.md) —
            // this app shipped that exact mistake once.
            ConnectableRow(name = "Sleep", connected = true)
            ConnectableRow(name = "Steps", connected = true)
            ConnectableRow(name = "Heart rate", connected = false)

            // §8 level ③ — navigation is a mono accent link, not a button.
            SettingsActionLink("Manage permissions →") { }

            // ── Page-level actions, GROUPED AT THE END ──────────────────────────────────────────
            // §8: never mid-scroll. ① filled is the do-it-now; ② outlined is its sidekick. A
            // destructive one-shot stays level ② tinted `error`, paired with an Undo snackbar —
            // never a filled red button.
            Spacer(Modifier.height(28.dp))
            SettingsActionRow {
                SettingsPrimaryAction("Update Health Connect") { }
                SettingsOutlineAction("Re-sync from watch") { }
            }
            Spacer(Modifier.height(32.dp))
        }
    }
}

/**
 * One connectable integration. Demonstrates §2③'s row-scoped action rule and §8's
 * "don't render state twice": the dot carries the state, so the right side shows a reading or an
 * action — never the word "Connected" repeating what the dot already said.
 */
@Composable
private fun ConnectableRow(name: String, connected: Boolean) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        Modifier
            .fillMaxWidth()
            // The WHOLE row is the tap target. The pill below is drawn, not separately clickable —
            // a nested tap is banned (§2③).
            .padding(horizontal = SETTINGS_GUTTER, vertical = SETTINGS_ROW_PAD),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // §8/§12: the dot rail. Filled disc = present, muted ring = absent — so the empty state is
        // DRAWN, not a column of the words "Not connected".
        StatusDot(active = connected)
        Column(Modifier.weight(1f)) {
            Text(name, style = MaterialTheme.typography.bodyMedium, color = onBg)
            if (connected) {
                // §4.9: show the reading, not just the conclusion. §12: a stale signal reports its
                // AGE rather than an error banner.
                SettingsExplainer("Last read 2h ago")
            }
        }
        if (!connected) ConnectPill()
    }
}

@Preview(name = "Settings", showBackground = true, backgroundColor = 0xFF0E0E11)
@Composable
private fun SettingsRecipePreview() {
    ForgeTheme { SettingsRecipe() }
}

@Preview(name = "Settings · 200% font", showBackground = true, backgroundColor = 0xFF0E0E11, fontScale = 2.0f)
@Composable
private fun SettingsRecipeLargeFontPreview() {
    ForgeTheme { SettingsRecipe() }
}
