package com.forge.app.ui.overview.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.bounceClick

/**
 * Big tappable card for an Overview hub destination. [primary] = the Gym variant
 * (larger, accent-colored gradient). Non-primary variants are smaller and use a
 * subtle accent-to-surface gradient so they still feel related but read as secondary.
 */
@Composable
fun NavTile(
    label: String,
    word: String,
    onClick: () -> Unit,
    primary: Boolean,
    modifier: Modifier = Modifier
) {
    val accent = MaterialTheme.colorScheme.primary
    val surface = MaterialTheme.colorScheme.surface
    val gradient = remember(primary, accent, surface) {
        if (primary) {
            Brush.linearGradient(
                colors = listOf(accent.copy(alpha = 0.28f), accent.copy(alpha = 0.10f))
            )
        } else {
            Brush.linearGradient(
                colors = listOf(surface, accent.copy(alpha = 0.08f))
            )
        }
    }

    Box(
        modifier = modifier
            .clip(RoundedCornerShape(16.dp))
            .background(gradient)
            .bounceClick { onClick() }
            .padding(20.dp)
    ) {
        Column(
            modifier = Modifier.fillMaxSize(),
            verticalArrangement = Arrangement.SpaceBetween
        ) {
            Text(
                text = word,
                color = accent,
                fontWeight = FontWeight.Black,
                fontSize = 10.sp,
                letterSpacing = 2.sp
            )
            Text(
                text = label,
                style = if (primary) MaterialTheme.typography.displayLarge else MaterialTheme.typography.headlineLarge,
                color = MaterialTheme.colorScheme.onSurface
            )
        }
    }
}

/**
 * Convenience wrapper that fixes a height. Caller still controls width — pass
 * `Modifier.fillMaxWidth()` for the full-width primary tile, or `Modifier.weight(1f)`
 * inside a Row for the side-by-side secondaries.
 */
@Composable
fun NavTile(
    label: String,
    word: String,
    onClick: () -> Unit,
    primary: Boolean,
    heightDp: Int,
    modifier: Modifier = Modifier
) {
    NavTile(
        label = label,
        word = word,
        onClick = onClick,
        primary = primary,
        modifier = modifier.height(heightDp.dp)
    )
}
