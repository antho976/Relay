package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.domain.adapt.EffortPrompt
import com.forge.app.ui.common.ForgeTileGrid
import com.forge.app.ui.common.memberFill
import com.forge.app.ui.common.memberShape
import com.forge.app.ui.theme.OpenType

/**
 * The one-tap effort question: "How many more could you have done?" over five connected keys,
 * 0 to 4+. One tap answers it and writes the RPE that count equals ([EffortPrompt.rpeFor]).
 *
 * It replaced the four-way "How did it feel?" rater (Easy, Just right, Hard, Brutal), which sat
 * folded away at the bottom of the card, and it stands in for the two-tap RPE cell on the set that
 * matters. Used in three places with the same keys: under the set just logged, on the finish
 * summary for anything left unrated, and on a finished session's page.
 *
 * @param rpe the set's current RPE, so an answered row shows its pick. Null draws no pick.
 * @param label the question, or the exercise's name where a list of them is asked at once.
 */
@Composable
fun RepsLeftRow(
    rpe: Double?,
    onPick: (rpe: Double) -> Unit,
    modifier: Modifier = Modifier,
    label: String = "How many more could you have done?"
) {
    val picked = EffortPrompt.repsLeftFor(rpe)
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(label, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onSurfaceVariant)
        // One connected pick-one group, like every other single choice in the session.
        ForgeTileGrid(EffortPrompt.CHOICES, cols = EffortPrompt.CHOICES.size) { repsLeft, corners, m ->
            val isSelected = picked == repsLeft
            val shape = memberShape(corners, isSelected)
            Box(
                modifier = m
                    .heightIn(min = 48.dp)
                    .memberFill(shape, isSelected)
                    .semantics {
                        role = Role.RadioButton
                        selected = isSelected
                    }
                    .clickable(onClickLabel = EffortPrompt.spoken(repsLeft)) { onPick(EffortPrompt.rpeFor(repsLeft)) }
                    .padding(vertical = 10.dp),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    EffortPrompt.label(repsLeft),
                    style = MaterialTheme.typography.titleMedium,
                    color = if (isSelected) MaterialTheme.colorScheme.onBackground
                    else MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}
