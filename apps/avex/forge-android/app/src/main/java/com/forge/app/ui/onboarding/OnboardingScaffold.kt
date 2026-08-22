package com.forge.app.ui.onboarding

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.clickableLabeled

/**
 * The flow's shell, the one thing every step shares: chrome on top, the question in the middle, the
 * plan under construction below it, one action at the bottom.
 *
 * It is a composable of its own so the parts that never change between steps are written once, and
 * so the whole page — chrome, ledger and CTA together, not a step in isolation — can be rendered
 * and looked at off-device (`OnboardingScreenshotTest`).
 *
 * [ledger] is invoked unconditionally and owns its own visibility, so it can animate itself in and
 * out (the leading air belongs inside it, or a hidden ledger leaves a gap behind).
 */
@Composable
internal fun OnboardingScaffold(
    step: Int,
    total: Int,
    onBack: (() -> Unit)?,
    onSkip: (() -> Unit)?,
    bottomBar: @Composable () -> Unit,
    ledger: @Composable () -> Unit = {},
    gateHint: String? = null,
    content: @Composable ColumnScope.() -> Unit
) {
    // No solid background — the theme's page gradient shows through, like every other screen.
    Box(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 24.dp, vertical = 28.dp)
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Top chrome: ← back, the step rail, skip →. One back affordance per page (§4.6).
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                if (onBack != null) {
                    Box(
                        modifier = Modifier
                            .size(48.dp)
                            .clickableLabeled("Back", onClick = onBack),
                        contentAlignment = Alignment.Center
                    ) {
                        Text("←", style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onBackground)
                    }
                } else {
                    Spacer(Modifier.width(48.dp))
                }
                StepRail(step = step, total = total, modifier = Modifier.weight(1f))
                if (onSkip != null) SkipLink(onSkip) else Spacer(Modifier.width(36.dp))
            }
            Spacer(Modifier.height(24.dp))

            content()

            ledger()
            Spacer(Modifier.height(16.dp))

            if (gateHint != null) {
                Text(
                    gateHint,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
                )
                Spacer(Modifier.height(10.dp))
            }
            bottomBar()
        }
    }
}
