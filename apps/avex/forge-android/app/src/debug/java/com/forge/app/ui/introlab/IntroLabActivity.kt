package com.forge.app.ui.introlab

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import com.forge.app.appicon.AppIcon
import com.forge.app.ui.common.AvexIntro
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.SegmentPill
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.launch.LaunchMascot
import com.forge.app.ui.theme.ForgeTheme
import com.forge.app.ui.theme.forgeBackgroundGradient

/**
 * Debug-only: play the cold-launch intro for any icon with any mascot (or none), on demand. The real
 * intro plays once per cold launch with a random mascot, which makes any one combination
 * impossible to review by just opening the app. Debug source set only; never ships.
 */
class IntroLabActivity : ComponentActivity() {
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
                    IntroLab()
                }
            }
        }
    }
}

@Composable
private fun IntroLab() {
    var icon by remember { mutableStateOf(AppIcon.Default) }
    var mascot by remember { mutableStateOf<LaunchMascot?>(LaunchMascot.Kettle) }
    var playing by remember { mutableStateOf(false) }
    var take by remember { mutableIntStateOf(0) }
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary

    LazyVerticalGrid(
        columns = GridCells.Fixed(5),
        modifier = Modifier.fillMaxSize().safeDrawingPadding(),
        contentPadding = PaddingValues(24.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item(span = { GridItemSpan(maxLineSpan) }) {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text("Intro lab", style = MaterialTheme.typography.headlineSmall, color = MaterialTheme.colorScheme.onBackground)
                EditorialHeader("Mascot", muted, accent)
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    val onBg = MaterialTheme.colorScheme.onBackground
                    val outline = MaterialTheme.colorScheme.outline
                    SegmentPill("None", mascot == null, { mascot = null }, accent, onBg, muted, outline)
                    LaunchMascot.entries.forEach { m ->
                        SegmentPill(m.name, mascot == m, { mascot = m }, accent, onBg, muted, outline)
                    }
                }
                ForgePrimaryCapsule(
                    label = "Play ${icon.displayName}",
                    onClick = { take++; playing = true },
                    modifier = Modifier.fillMaxWidth(),
                )
                EditorialHeader("Icon", muted, accent)
            }
        }
        items(AppIcon.entries, key = { it.name }) { entry ->
            val shape = RoundedCornerShape(12.dp)
            Image(
                painter = painterResource(entry.previewRes),
                contentDescription = entry.displayName,
                modifier = Modifier
                    .aspectRatio(1f)
                    .clip(shape)
                    .border(if (entry == icon) 2.dp else 0.dp, accent, shape)
                    .bounceClick(onClick = { icon = entry }),
            )
        }
    }

    if (playing) {
        key(take) {
            AvexIntro(
                iconKey = icon.name,
                themed = true,
                mascot = mascot,
                rollMascot = false,
                onDone = { playing = false },
            )
        }
    }
}
