package com.forge.app.ui.recipes

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import com.forge.app.ui.theme.ForgeTheme
import com.forge.app.ui.theme.forgeBackgroundGradient

/**
 * Debug-only launcher for the archetype recipes (DESIGN §3).
 *
 * The recipes previewed fine in the IDE but were unreachable in a running app, which made "look at
 * your design system on a real device, at the real system font scale" untrue. This adds a second
 * launcher icon in debug builds only — the debug variant already has its own applicationId suffix,
 * so it never collides with the installed app, and nothing here exists in release.
 *
 * Deliberately not wired into `ForgeNavHost`: the nav host lives in `main` and must not know that
 * these exist.
 */
class RecipeGalleryActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            ForgeTheme {
                val (top, bottom) = forgeBackgroundGradient(amoled = false)
                Box(
                    Modifier
                        .fillMaxSize()
                        .background(Brush.verticalGradient(listOf(top, bottom)))
                ) {
                    RecipeGalleryScreen()
                }
            }
        }
    }
}
