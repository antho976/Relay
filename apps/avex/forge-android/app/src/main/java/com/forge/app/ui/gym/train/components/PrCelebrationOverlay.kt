package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.ConfettiOverlay

/** Full-screen PR celebration: dark scrim + lift name/weight + confetti. */
@Composable
fun PrCelebrationOverlay(
    exerciseName: String,
    pbText: String?,
    onComplete: () -> Unit
) {
    val accent = MaterialTheme.colorScheme.primary
    Box(
        modifier = Modifier
            .fillMaxSize()
            .clearAndSetSemantics { contentDescription = "Personal record: $exerciseName${pbText?.let { " · $it" } ?: ""}" }
    ) {
        // Dark scrim
        Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.82f)))
        // Text content — centred vertically and horizontally
        Column(
            modifier = Modifier
                .align(Alignment.Center)
                .padding(horizontal = 32.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Text(
                "PERSONAL RECORD",
                style = MaterialTheme.typography.titleMedium,
                color = accent,
                fontWeight = FontWeight.SemiBold,
                letterSpacing = androidx.compose.ui.unit.TextUnit(2f, androidx.compose.ui.unit.TextUnitType.Sp),
                textAlign = TextAlign.Center
            )
            Spacer(Modifier.height(16.dp))
            Text(
                exerciseName,
                style = MaterialTheme.typography.displaySmall,
                color = Color.White,
                fontStyle = FontStyle.Normal,
                fontWeight = FontWeight.Bold,
                textAlign = TextAlign.Center
            )
            pbText?.let { pb ->
                Spacer(Modifier.height(12.dp))
                Text(
                    pb,
                    style = MaterialTheme.typography.headlineMedium,
                    color = accent,
                    fontStyle = FontStyle.Italic,
                    textAlign = TextAlign.Center
                )
            }
        }
        // Confetti on top — drives the auto-dismiss
        ConfettiOverlay(modifier = Modifier.fillMaxSize(), onComplete = onComplete)
    }
}
