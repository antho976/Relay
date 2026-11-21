package com.forge.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.gym.train.components.RepsLeftRow
import com.forge.app.ui.theme.DayIcons
import com.forge.app.ui.theme.ForgeTheme
import com.forge.app.ui.theme.OpenType
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Goldens for two pieces added on 2026-11-18 that no screen golden reaches.
 *
 * The effort row sits inside the live session's exercise card, the finish summary and a finished
 * session's page, none of which a golden draws with a set just logged. It is five keys in a row at
 * any font scale, which is exactly the shape that clips at 200%, so both ends are pinned.
 *
 * The day body maps are filled glyphs whose whole job is that the lit part reads at 30dp against
 * the rest at a quarter strength, in Home's own quiet tile. A change to either alpha or to a path
 * would pass every unit test and be visible only here.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = RobolectricDeviceQualifiers.Pixel7)
class EffortAndDayIconsScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private val options = RoborazziOptions(
        compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f)
    )

    private fun shoot(name: String, fontScale: Float = 1f, content: @Composable () -> Unit) {
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(LocalDensity provides Density(base.density, fontScale)) {
                ForgeTheme {
                    Box(Modifier.background(MaterialTheme.colorScheme.background).padding(24.dp)) { content() }
                }
            }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png", options)
    }

    @Composable
    private fun effortRows() {
        Column(verticalArrangement = Arrangement.spacedBy(28.dp)) {
            // As it is asked under the set just logged, then answered at two left, then at the open end.
            RepsLeftRow(rpe = null, onPick = {})
            RepsLeftRow(rpe = 8.0, onPick = {})
            RepsLeftRow(rpe = 6.0, onPick = {}, label = "Seated Cable Row")
        }
    }

    @Test fun effortRow() = shoot("effort-row") { effortRows() }
    @Test fun effortRowLarge() = shoot("effort-row-200", fontScale = 2f) { effortRows() }

    @Test
    fun dayIcons() = shoot("day-icons") {
        val days = listOf(
            "Upper A" to DayIcons.Upper, "Lower B" to DayIcons.Lower, "Push" to DayIcons.Push,
            "Pull" to DayIcons.Pull, "Full Body" to DayIcons.FullBody, "Core" to DayIcons.Core
        )
        Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(18.dp)) {
            days.forEach { (name, glyph) ->
                Row(horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically) {
                    // Home's recent-row tile and glyph size, exactly.
                    Box(
                        Modifier.size(36.dp).background(MaterialTheme.colorScheme.surfaceContainerHigh, RoundedCornerShape(4.dp)),
                        contentAlignment = Alignment.Center
                    ) {
                        Icon(glyph, contentDescription = null, tint = MaterialTheme.colorScheme.onBackground, modifier = Modifier.size(30.dp))
                    }
                    Text(name, style = OpenType.rowTitle, color = MaterialTheme.colorScheme.onBackground, modifier = Modifier.weight(1f))
                    OpenLabel("Wed 18", style = OpenType.monoXS)
                }
            }
        }
    }

    /** The name-to-glyph rule Home uses, since a wrong match would draw the wrong body. */
    @Test
    fun dayNamesPickTheirBodyMap() {
        org.junit.Assert.assertSame(DayIcons.Upper, DayIcons.forDayName("Upper A"))
        org.junit.Assert.assertSame(DayIcons.Lower, DayIcons.forDayName("Legs B"))
        org.junit.Assert.assertSame(DayIcons.Lower, DayIcons.forDayName("Lower A"))
        org.junit.Assert.assertSame(DayIcons.Push, DayIcons.forDayName("Push A"))
        org.junit.Assert.assertSame(DayIcons.Pull, DayIcons.forDayName("Pull B"))
        org.junit.Assert.assertSame(DayIcons.FullBody, DayIcons.forDayName("Full Body C"))
        org.junit.Assert.assertSame(DayIcons.Core, DayIcons.forDayName("Core & Calves"))
        org.junit.Assert.assertNull(DayIcons.forDayName("Freestyle"))
    }
}
