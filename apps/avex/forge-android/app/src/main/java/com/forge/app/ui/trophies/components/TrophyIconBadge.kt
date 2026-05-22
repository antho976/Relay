package com.forge.app.ui.trophies.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Filter4
import androidx.compose.material.icons.filled.FitnessCenter
import androidx.compose.material.icons.filled.Layers
import androidx.compose.material.icons.filled.LocalFireDepartment
import androidx.compose.material.icons.filled.Login
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material.icons.filled.WorkspacePremium
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.program.TrophyIcon

/**
 * Round badge with a Material icon at its centre. Phase 6 uses stock Material icons —
 * the React prototype's hand-drawn SVGs are a Phase 8 polish swap (the [TrophyIcon] enum
 * is the stable interface, this is just one renderer).
 */
@Composable
fun TrophyIconBadge(
    icon: TrophyIcon,
    unlocked: Boolean,
    modifier: Modifier = Modifier,
    size: Dp = 48.dp
) {
    val accent = MaterialTheme.colorScheme.primary
    val bg = if (unlocked) accent.copy(alpha = 0.20f) else MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.30f)
    val fg = if (unlocked) accent else MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.55f)
    Box(
        modifier = modifier
            .size(size)
            .clip(CircleShape)
            .background(bg),
        contentAlignment = Alignment.Center
    ) {
        Icon(
            imageVector = vectorFor(icon),
            contentDescription = null,
            tint = fg,
            modifier = Modifier.size(size * 0.55f)
        )
    }
}

private fun vectorFor(icon: TrophyIcon): ImageVector = when (icon) {
    TrophyIcon.SPARK -> Icons.Filled.AutoAwesome
    TrophyIcon.STAR -> Icons.Filled.Star
    TrophyIcon.FLAME -> Icons.Filled.LocalFireDepartment
    TrophyIcon.SWAP -> Icons.Filled.SwapHoriz
    TrophyIcon.CHECK -> Icons.Filled.CheckCircle
    TrophyIcon.STACK -> Icons.Filled.Layers
    TrophyIcon.CROWN -> Icons.Filled.WorkspacePremium
    TrophyIcon.DOOR -> Icons.Filled.Login
    TrophyIcon.FOUR -> Icons.Filled.Filter4
    TrophyIcon.DUMBBELL -> Icons.Filled.FitnessCenter
    TrophyIcon.BOLT -> Icons.Filled.Bolt
}
