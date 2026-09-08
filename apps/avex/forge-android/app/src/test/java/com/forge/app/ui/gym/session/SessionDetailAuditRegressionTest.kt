package com.forge.app.ui.gym.session

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import com.forge.app.ui.gym.session.state.*
import com.forge.app.ui.gym.stats.components.*
import com.forge.app.ui.gym.train.components.UpNextBubble
import com.forge.app.ui.theme.ForgeTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk=[34], qualifiers="w360dp-h800dp-mdpi")
class SessionDetailAuditRegressionTest {
    @get:Rule val compose = createComposeRule()
    @Test fun `timed only session exposes saved exercise in all lenses`() {
        val metric = mutableStateOf(SessionMetric.WEIGHT)
        val hold = SetDetail(1, null, "BW", 0, null, false, false, false, false, null, null, 90)
        val exercise = ExerciseDetail("Plank", false, null, true, null, null, 0, 0.0, null, false, listOf(hold))
        compose.setContent { ForgeTheme { Column {
            val c = MaterialTheme.colorScheme
            if(metric.value == SessionMetric.RPE) RpeExerciseCard(listOf(exercise), SessionChartStyle.BARS, {}, c.onBackground,c.onSurfaceVariant,c.primary,c.outline)
            else MetricExerciseCard(listOf(exercise),metric.value,SessionChartStyle.BARS,{},c.onBackground,c.onSurfaceVariant,c.primary,c.outline)
        } } }
        for (m in SessionMetric.entries) {
            compose.runOnIdle { metric.value = m }
            compose.onNodeWithText("Plank").assertExists()
        }
    }
    @Test fun `last exercise can open add exercise action`() {
        compose.setContent { ForgeTheme {
            UpNextBubble(null,null,null,emptyList(),{}, {}, {})
        } }
        compose.onNodeWithText("UP NEXT", useUnmergedTree=true).performClick()
        compose.onNodeWithText("+ add exercise").assertExists()
    }
    @Test fun `all anatomy paths parse using production Compose parser`() {
        val paths = (ANATOMY_FRONT+ANATOMY_BACK).flatMap { it.paths } + listOf(ANATOMY_FRONT_OUTLINE,ANATOMY_BACK_OUTLINE)
        paths.forEach { path ->
            val nodes = PathParser().parsePathString(path).toNodes()
            assertTrue(nodes.isNotEmpty())
        }
    }
}
