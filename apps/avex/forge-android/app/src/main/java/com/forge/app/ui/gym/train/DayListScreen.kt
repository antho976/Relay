package com.forge.app.ui.gym.train

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.gym.stats.StatsContent
import com.forge.app.ui.gym.train.components.DayCard

/**
 * Gym hub. Hosts two tabs: **Train** (the day list, Phase 3) and **Stats** (Phase 5).
 * Tab selection is preserved via rememberSaveable so backing out and returning keeps you
 * on whichever subtab you last had open.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DayListScreen(
    onBack: () -> Unit,
    onOpenDay: (String) -> Unit,
    onOpenDayQuick: (String) -> Unit,
    viewModel: DayListViewModel = hiltViewModel()
) {
    var selectedTab by rememberSaveable { mutableIntStateOf(0) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("GYM", style = MaterialTheme.typography.headlineLarge) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                    titleContentColor = MaterialTheme.colorScheme.onBackground
                )
            )
        },
        containerColor = MaterialTheme.colorScheme.background
    ) { inner ->
        Column(Modifier.fillMaxSize().padding(inner)) {
            TabRow(
                selectedTabIndex = selectedTab,
                containerColor = MaterialTheme.colorScheme.background,
                contentColor = MaterialTheme.colorScheme.primary
            ) {
                Tab(
                    selected = selectedTab == 0,
                    onClick = { selectedTab = 0 },
                    text = { Text("Train") }
                )
                Tab(
                    selected = selectedTab == 1,
                    onClick = { selectedTab = 1 },
                    text = { Text("Stats") }
                )
            }

            Box(Modifier.fillMaxSize()) {
                when (selectedTab) {
                    0 -> TrainTab(
                        onOpenDay = onOpenDay,
                        onOpenDayQuick = onOpenDayQuick,
                        viewModel = viewModel
                    )
                    1 -> StatsContent(modifier = Modifier.fillMaxSize())
                }
            }
        }
    }
}

@Composable
private fun TrainTab(
    onOpenDay: (String) -> Unit,
    onOpenDayQuick: (String) -> Unit,
    viewModel: DayListViewModel
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(
            "Pick your day.",
            style = MaterialTheme.typography.headlineMedium,
            color = MaterialTheme.colorScheme.onSurface
        )
        state.days.forEach { item ->
            DayCard(
                item = item,
                onClick = { onOpenDay(item.plan.key) },
                onQuickStart = if (item.isNextUp && !item.isActive) {
                    { onOpenDayQuick(item.plan.key) }
                } else null
            )
        }
    }
}
