package com.forge.app.ui.onboarding

import com.forge.app.ui.common.ForgeChromeButton
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.ui.draw.clip
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
                    ForgeChromeButton(onClick = onBack, label = "Back") {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onBackground,
                            modifier = Modifier.size(20.dp)
                        )
                    }
                } else {
                    Spacer(Modifier.width(44.dp))
                }
                StepRail(step = step, total = total, modifier = Modifier.weight(1f))
                if (onSkip != null) {
                    ForgeChromeButton(onClick = onSkip, label = "Skip setup") {
                        Text(
                            "Skip",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(horizontal = 14.dp)
                        )
                    }
                } else {
                    Spacer(Modifier.width(44.dp))
                }
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
