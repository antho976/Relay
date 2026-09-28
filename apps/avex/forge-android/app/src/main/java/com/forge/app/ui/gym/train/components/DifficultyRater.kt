package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.layout.heightIn
import com.forge.app.ui.common.Corners
import com.forge.app.ui.common.ForgeTileGrid
import com.forge.app.ui.common.memberFill
import com.forge.app.ui.common.memberShape

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.types.EffortRating

/**
 * Four-way effort rating row shown at the bottom of an expanded exercise card once
 * at least one set has been logged. Tapping a rating writes through to the
 * corresponding [com.forge.app.data.db.entities.LoggedExercise].
 */
@Composable
fun DifficultyRater(
    selected: EffortRating?,
    onSelect: (EffortRating) -> Unit,
    modifier: Modifier = Modifier
) {
    // 2×2 grid so labels like "Just Right" never overflow on narrow phones.
    Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        // One connected pick-one group of tiles (2dp seams); the pick rounds out and wears the ring.
        val ratings = EffortRating.entries
        ForgeTileGrid(ratings, cols = 2) { rating, corners, m ->
            RatingChip(
                rating = rating,
                isSelected = selected == rating,
                corners = corners,
                onClick = { onSelect(rating) },
                modifier = m
            )
        }
    }
}

@Composable
private fun RatingChip(
    rating: EffortRating,
    isSelected: Boolean,
    corners: Corners,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val textColor = if (isSelected) MaterialTheme.colorScheme.onBackground
                    else MaterialTheme.colorScheme.onSurfaceVariant
    val shape = memberShape(corners, isSelected)

    Row(
        modifier = modifier
            .heightIn(min = 48.dp)
            .memberFill(shape, isSelected)
            .semantics {
                role = Role.RadioButton
                this.selected = isSelected
            }
            .clickable(onClickLabel = rating.displayName, onClick = onClick)
            .padding(vertical = 12.dp, horizontal = 8.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text = rating.displayName,
            color = textColor,
            style = MaterialTheme.typography.labelMedium,
            textAlign = TextAlign.Center
        )
    }
}
