@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.recipes

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeWordmark
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.ForgeTheme

/**
 * Debug-only browser for the six archetype recipes (DESIGN §3).
 *
 * IDE previews are fine for a glance, but a design system that is only ever seen in a preview pane
 * is not really being looked at. This puts the reference archetypes on a real device, at the real
 * system font scale, in the real theme — which is where taste calls actually get made.
 *
 * Debug source set only: it never ships. Wire it from a debug entry point when you want it; it is
 * deliberately not routed, so it cannot leak into the nav graph.
 */
enum class Recipe(val label: String, val blurb: String) {
    OVERVIEW("Overview", "Home, Stats, Cardio, Coach, Profile"),
    DETAIL("Detail", "one session, one lift, one entry"),
    LIST("List", "History, Goals, Trophies, pickers"),
    SETTINGS("Settings", "settings, editors, onboarding"),
    LIVE("Live", "live session, freestyle log"),
    MODAL("Modal", "sheets and dialogs"),
}

@Composable
fun RecipeGalleryScreen() {
    var open by remember { mutableStateOf<Recipe?>(null) }

    when (open) {
        null -> RecipeIndex(onPick = { open = it })
        Recipe.OVERVIEW -> BackTo({ open = null }) { OverviewRecipe() }
        Recipe.DETAIL -> DetailRecipe(onBack = { open = null })
        Recipe.LIST -> ListRecipe(onBack = { open = null })
        Recipe.SETTINGS -> SettingsRecipe(onBack = { open = null })
        Recipe.LIVE -> BackTo({ open = null }) { LiveRecipe() }
        Recipe.MODAL -> BackTo({ open = null }) { ModalRecipeContent(onDismiss = { open = null }) }
    }
}

/** The hub archetypes have no back arrow of their own (they are tabs), so the gallery adds one. */
@Composable
private fun BackTo(onBack: () -> Unit, content: @Composable () -> Unit) {
    Column(Modifier.fillMaxSize()) {
        Text(
            "← recipes",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier
                .bounceClick(onClick = onBack)
                .padding(horizontal = 24.dp, vertical = 12.dp)
        )
        content()
    }
}

@Composable
private fun RecipeIndex(onPick: (Recipe) -> Unit) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    Scaffold(
        topBar = {
            TopAppBar(
                title = { ForgeWordmark() },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        Column(
            Modifier
                .padding(inner)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp)
        ) {
            Spacer(Modifier.height(8.dp))
            Text("Recipes", style = MaterialTheme.typography.headlineLarge, color = onBg)
            Spacer(Modifier.height(2.dp))
            Text(
                "${Recipe.entries.size} ARCHETYPES",
                style = MaterialTheme.typography.labelMedium,
                color = muted
            )

            Spacer(Modifier.height(28.dp))
            EditorialHeader("ARCHETYPES", muted, accent)
            Spacer(Modifier.height(10.dp))

            Recipe.entries.forEach { r ->
                Row(
                    Modifier
                        .fillMaxWidth()
                        .bounceClick { onPick(r) }
                        .padding(vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(r.label, style = MaterialTheme.typography.bodyMedium, color = onBg)
                        Text(
                            r.blurb,
                            style = MaterialTheme.typography.labelSmall,
                            color = muted.copy(alpha = 0.65f)
                        )
                    }
                    Text("→", style = MaterialTheme.typography.bodyMedium, color = accent)
                }
            }
            Spacer(Modifier.height(32.dp))
        }
    }
}

@Preview(name = "Recipe gallery", showBackground = true, backgroundColor = 0xFF0E0E11)
@Composable
private fun RecipeGalleryPreview() {
    ForgeTheme { RecipeGalleryScreen() }
}

@Preview(name = "Recipe gallery · 200% font", showBackground = true, backgroundColor = 0xFF0E0E11, fontScale = 2.0f)
@Composable
private fun RecipeGalleryLargeFontPreview() {
    ForgeTheme { RecipeGalleryScreen() }
}
