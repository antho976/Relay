package com.forge.app.ui.trophies

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.trophies.components.TrophyIconBadge
import com.forge.app.ui.trophies.state.TrophiesUiState
import com.forge.app.ui.trophies.state.TrophyDisplay
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TrophiesScreen(
    onBack: () -> Unit,
    viewModel: TrophiesViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val bg = MaterialTheme.colorScheme.background

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        Text("•", style = MaterialTheme.typography.bodyMedium, color = muted)
                        Text(
                            "Forge",
                            style = MaterialTheme.typography.bodyMedium,
                            color = onBg,
                            fontStyle = FontStyle.Italic
                        )
                    }
                },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back",
                            tint = muted
                        )
                    }
                },
                actions = {
                    Text(
                        "TROPHIES",
                        style = MaterialTheme.typography.labelSmall,
                        letterSpacing = 2.sp,
                        color = muted,
                        modifier = Modifier.padding(end = 16.dp)
                    )
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        val nextLocked = remember(state.sections) {
            state.sections.flatMap { it.displays }.firstOrNull { !it.isUnlocked }
        }

        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(bottom = 56.dp)
        ) {
            item("hero") {
                HeroSection(
                    state = state,
                    nextLocked = nextLocked,
                    onBg = onBg,
                    muted = muted,
                    outline = outline
                )
            }

            state.sections.forEach { section ->
                item("h-${section.category.code}") {
                    Column(Modifier.padding(horizontal = 24.dp)) {
                        Spacer(Modifier.height(24.dp))
                        Text(
                            section.category.displayName,
                            style = MaterialTheme.typography.headlineSmall,
                            color = onBg,
                            fontStyle = FontStyle.Italic
                        )
                        Spacer(Modifier.height(12.dp))
                    }
                }
                items(section.displays, key = { it.trophy.id }) { display ->
                    TrophyRow(
                        display = display,
                        onBg = onBg,
                        muted = muted,
                        bg = bg
                    )
                    HorizontalDivider(
                        modifier = Modifier.padding(horizontal = 24.dp),
                        color = outline.copy(alpha = 0.18f)
                    )
                }
            }
        }
    }
}

@Composable
private fun HeroSection(
    state: TrophiesUiState,
    nextLocked: TrophyDisplay?,
    onBg: Color,
    muted: Color,
    outline: Color
) {
    val frac = if (state.totalCount == 0) 0f
               else state.unlockedCount.toFloat() / state.totalCount

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp)
            .padding(top = 8.dp, bottom = 16.dp)
    ) {
        Text(
            "${state.totalCount} IN ALL · ${state.unlockedCount} EARNED",
            style = MaterialTheme.typography.labelSmall,
            color = muted,
            fontSize = 9.sp,
            letterSpacing = 1.sp
        )
        Spacer(Modifier.height(8.dp))
        Text(
            "${numberWord(state.unlockedCount)} of ${numberWord(state.totalCount)}.",
            style = MaterialTheme.typography.displayLarge,
            color = onBg
        )
        Spacer(Modifier.height(14.dp))
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(2.dp)
                .background(outline.copy(alpha = 0.2f))
        ) {
            Box(
                modifier = Modifier
                    .fillMaxWidth(frac.coerceIn(0f, 1f))
                    .fillMaxHeight()
                    .background(onBg.copy(alpha = 0.7f))
            )
        }
        if (nextLocked != null) {
            Spacer(Modifier.height(12.dp))
            val nudgeText = buildAnnotatedString {
                append("Up next: ")
                withStyle(SpanStyle(fontWeight = FontWeight.Bold)) {
                    append(nextLocked.trophy.name)
                }
                append(" — ")
                append(nextLocked.trophy.description.replaceFirstChar { it.lowercase() })
                append(".")
            }
            Text(
                nudgeText,
                style = MaterialTheme.typography.bodySmall,
                color = muted,
                fontStyle = FontStyle.Italic
            )
        }
        Spacer(Modifier.height(16.dp))
        HorizontalDivider(color = outline.copy(alpha = 0.25f))
    }
}

@Composable
private fun TrophyRow(
    display: TrophyDisplay,
    onBg: Color,
    muted: Color,
    bg: Color,
    modifier: Modifier = Modifier
) {
    val unlocked = display.isUnlocked
    val nameColor = if (unlocked) onBg else muted.copy(alpha = 0.65f)
    val descColor = muted.copy(alpha = if (unlocked) 0.6f else 0.45f)

    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp)
    ) {
        Box(contentAlignment = Alignment.BottomEnd) {
            TrophyIconBadge(icon = display.trophy.icon, unlocked = unlocked, size = 40.dp)
            if (unlocked) {
                Box(
                    modifier = Modifier
                        .size(17.dp)
                        .clip(CircleShape)
                        .background(bg),
                    contentAlignment = Alignment.Center
                ) {
                    Box(
                        modifier = Modifier
                            .size(14.dp)
                            .clip(CircleShape)
                            .background(onBg),
                        contentAlignment = Alignment.Center
                    ) {
                        Icon(
                            Icons.Filled.Check,
                            contentDescription = null,
                            tint = bg,
                            modifier = Modifier.size(9.dp)
                        )
                    }
                }
            }
        }

        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                display.trophy.name,
                style = MaterialTheme.typography.bodyMedium,
                color = nameColor
            )
            Text(
                display.trophy.description,
                style = MaterialTheme.typography.bodySmall,
                color = descColor,
                fontStyle = FontStyle.Italic,
                fontSize = 11.sp
            )
        }

        if (unlocked) {
            val dateText = remember(display.unlockedAt) {
                "· " + SimpleDateFormat("MMM d", Locale.getDefault())
                    .format(Date(display.unlockedAt!!)).uppercase()
            }
            Text(
                dateText,
                style = MaterialTheme.typography.labelSmall,
                color = muted,
                fontSize = 9.sp
            )
        } else {
            Text(
                display.progressHint ?: "LOCKED",
                style = MaterialTheme.typography.labelSmall,
                color = muted.copy(alpha = 0.4f),
                fontSize = 9.sp
            )
        }
    }
}

private fun numberWord(n: Int): String = when (n) {
    0 -> "Zero"; 1 -> "One"; 2 -> "Two"; 3 -> "Three"; 4 -> "Four"; 5 -> "Five"
    6 -> "Six"; 7 -> "Seven"; 8 -> "Eight"; 9 -> "Nine"; 10 -> "Ten"
    11 -> "Eleven"; 12 -> "Twelve"; 13 -> "Thirteen"; 14 -> "Fourteen"; 15 -> "Fifteen"
    16 -> "Sixteen"; 17 -> "Seventeen"; 18 -> "Eighteen"; 19 -> "Nineteen"; 20 -> "Twenty"
    21 -> "Twenty-one"; 22 -> "Twenty-two"; 23 -> "Twenty-three"; 24 -> "Twenty-four"
    25 -> "Twenty-five"; 26 -> "Twenty-six"; 27 -> "Twenty-seven"; 28 -> "Twenty-eight"
    29 -> "Twenty-nine"; 30 -> "Thirty"; 31 -> "Thirty-one"; 32 -> "Thirty-two"
    else -> n.toString()
}
