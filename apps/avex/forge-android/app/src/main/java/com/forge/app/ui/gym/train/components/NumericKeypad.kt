package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

private val KEY_HEIGHT = 50.dp

/**
 * In-app numeric keypad for logging sets. Replaces the system IME so logging a set never
 * triggers the keyboard's slide in/out — the single biggest source of input jank on the
 * training screen. Digits + decimal feed whichever field is active in [SetInputRow]; the
 * action keys handle backspace, weight→reps jump, bodyweight, and submit.
 *
 * Text-weight entries beyond "BW" (e.g. "2 plates") aren't reachable here — those are a
 * rare edge case; numeric + BW covers the overwhelming majority of logging.
 */
@Composable
fun NumericKeypad(
    onDigit: (Char) -> Unit,
    onDecimal: () -> Unit,
    onBackspace: () -> Unit,
    onNext: () -> Unit,
    onBodyweight: () -> Unit,
    onSubmit: () -> Unit,
    submitEnabled: Boolean,
    submitLabel: String,
    modifier: Modifier = Modifier
) {
    Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        KeypadRow {
            DigitKey("1", onDigit); DigitKey("2", onDigit); DigitKey("3", onDigit)
            ActionKey("⌫", onBackspace)
        }
        KeypadRow {
            DigitKey("4", onDigit); DigitKey("5", onDigit); DigitKey("6", onDigit)
            ActionKey("BW", onBodyweight)
        }
        KeypadRow {
            DigitKey("7", onDigit); DigitKey("8", onDigit); DigitKey("9", onDigit)
            ActionKey("→", onNext)
        }
        KeypadRow {
            ActionKey(".", onDecimal)
            DigitKey("0", onDigit)
            SubmitKey(submitLabel, submitEnabled, onSubmit)
        }
    }
}

@Composable
private fun KeypadRow(content: @Composable RowScope.() -> Unit) =
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        content = content
    )

@Composable
private fun RowScope.DigitKey(d: String, onDigit: (Char) -> Unit) =
    KeyCell(label = d, emphasized = false, modifier = Modifier.weight(1f)) { onDigit(d[0]) }

@Composable
private fun RowScope.ActionKey(label: String, onClick: () -> Unit) =
    KeyCell(label = label, emphasized = true, modifier = Modifier.weight(1f), onClick = onClick)

@Composable
private fun RowScope.SubmitKey(label: String, enabled: Boolean, onClick: () -> Unit) {
    Box(
        modifier = Modifier
            .weight(2f)
            .height(KEY_HEIGHT)
            .background(
                if (enabled) Color.White else Color.White.copy(alpha = 0.5f),
                RoundedCornerShape(10.dp)
            )
            .then(if (enabled) Modifier.clickable { onClick() } else Modifier),
        contentAlignment = Alignment.Center
    ) {
        Text(
            label,
            style = MaterialTheme.typography.labelLarge,
            color = Color.Black.copy(alpha = if (enabled) 1f else 0.5f)
        )
    }
}

@Composable
private fun KeyCell(
    label: String,
    emphasized: Boolean,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    Box(
        modifier = modifier
            .height(KEY_HEIGHT)
            .border(1.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(10.dp))
            .clickable { onClick() },
        contentAlignment = Alignment.Center
    ) {
        Text(
            label,
            style = if (emphasized) MaterialTheme.typography.titleMedium else MaterialTheme.typography.headlineSmall,
            color = if (emphasized) muted else onBg
        )
    }
}
