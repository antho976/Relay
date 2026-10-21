package com.forge.app.ui.onboarding

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType

/**
 * The flow's shell, the one thing every step shares (redrawn 2026-10-21): a 48dp chrome row (← back,
 * the step rail, a quiet mono SKIP), the step's own page in the middle, and a footer with the plan so
 * far over one red action.
 *
 * It is a composable of its own so the parts that never change between steps are written once, and
 * so the whole page — chrome, page and footer together, not a step in isolation — can be rendered
 * and looked at off-device (`OnboardingScreenshotTest`).
 *
 * The back arrow and SKIP keep their places when a step has neither (the first step has no back,
 * the last no skip), so the rail never shifts between steps.
 */
@Composable
internal fun OnboardingScaffold(
    step: Int,
    total: Int,
    onBack: (() -> Unit)?,
    onSkip: (() -> Unit)?,
    planSoFar: String?,
    bottomBar: @Composable () -> Unit,
    gateHint: String? = null,
    content: @Composable ColumnScope.() -> Unit
) {
    // No solid background: the theme's page gradient shows through, like every other screen.
    Column(
        modifier = Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .padding(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 20.dp)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth().height(48.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp)
        ) {
            val muted = MaterialTheme.colorScheme.onSurfaceVariant
            Box(
                Modifier
                    .size(22.dp)
                    .alpha(if (onBack != null) 1f else 0f)
                    .then(
                        if (onBack != null) Modifier
                            .touchSlop(vertical = 13.dp, horizontal = 13.dp)
                            .bounceClick(role = Role.Button, onClick = onBack)
                            .padding(13.dp)
                        else Modifier
                    ),
                contentAlignment = Alignment.Center
            ) {
                Icon(OpenIcons.ArrowLeft, contentDescription = if (onBack != null) "Back" else null, tint = muted)
            }
            StepRail(step = step, total = total, modifier = Modifier.weight(1f))
            Box(
                Modifier
                    .alpha(if (onSkip != null) 1f else 0f)
                    .then(
                        if (onSkip != null) Modifier
                            .touchSlop(vertical = 16.dp, horizontal = 8.dp)
                            .bounceClick(role = Role.Button, onClick = onSkip)
                            .padding(vertical = 16.dp, horizontal = 8.dp)
                        else Modifier.clearAndSetSemantics {}
                    )
            ) {
                OpenLabel("Skip")
            }
        }
        Spacer(Modifier.height(28.dp))

        Box(Modifier.weight(1f).fillMaxWidth()) {
            Column(Modifier.fillMaxSize(), content = content)
        }

        Spacer(Modifier.height(16.dp))
        if (gateHint != null) {
            Text(gateHint, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(10.dp))
        }
        if (planSoFar != null) {
            PlanSoFar(planSoFar)
            Spacer(Modifier.height(12.dp))
        }
        bottomBar()
    }
}
