package com.forge.app.ui.overview.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

/** "Welcome back — it's been X days" banner shown after a 5+ day training gap (#57). */
@Composable
fun ComebackBanner(
    daysSince: Int,
    modifier: Modifier = Modifier
) {
    val accent = MaterialTheme.colorScheme.primary
    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(accent.copy(alpha = 0.10f))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        Text(
            text = "WELCOME BACK",
            color = accent,
            fontWeight = FontWeight.Black,
            style = MaterialTheme.typography.labelLarge
        )
        Text(
            text = "It's been $daysSince days since your last session. Good to have you back.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface
        )
    }
}

/** Warning shown after 3+ consecutive training days (#58). */
@Composable
fun ConsecutiveDayWarning(
    consecutiveDays: Int,
    modifier: Modifier = Modifier
) {
    val warning = MaterialTheme.colorScheme.error
    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(warning.copy(alpha = 0.10f))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        Text(
            text = "$consecutiveDays DAYS STRAIGHT",
            color = warning,
            fontWeight = FontWeight.Black,
            style = MaterialTheme.typography.labelLarge
        )
        Text(
            text = "You've trained $consecutiveDays days in a row. Consider a rest day — recovery is where the gains happen.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface
        )
    }
}
