package com.forge.app.ui.trophies

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.trophies.state.TrophiesUiState

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

            item("filters") {
                FilterChips(
                    selected = state.selectedFilter,
                    onSelect = { viewModel.setFilter(it) },
                    onBg = onBg,
                    muted = muted,
                    outline = outline
                )
            }

            state.filteredSections.forEach { section ->
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
                        bg = bg,
                        outline = outline
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

