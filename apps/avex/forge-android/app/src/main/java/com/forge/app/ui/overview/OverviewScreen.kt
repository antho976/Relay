package com.forge.app.ui.overview

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.ui.overview.components.DeloadBanner
import com.forge.app.ui.overview.components.NavTile
import com.forge.app.ui.overview.components.WeeklyStatsStrip

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OverviewScreen(
    onGoToGym: () -> Unit,
    onGoToCardio: () -> Unit,
    onGoToTrophies: () -> Unit,
    viewModel: OverviewViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        text = "FORGE",
                        fontSize = 28.sp,
                        fontWeight = FontWeight.Black,
                        letterSpacing = 4.sp,
                        color = MaterialTheme.colorScheme.onBackground
                    )
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background
                )
            )
        },
        containerColor = MaterialTheme.colorScheme.background
    ) { inner ->
        Box(Modifier.fillMaxSize().padding(inner)) {
            Column(
                modifier = Modifier
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 16.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                WeeklyStatsStrip(
                    workouts = state.workoutsThisWeek,
                    volumeLb = state.volumeThisWeekLb,
                    cardioMinutes = state.cardioMinutesThisWeek
                )

                if (state.needsDeload) {
                    DeloadBanner(
                        sessionsSinceLast = state.sessionsSinceLastDeload,
                        onMarkDeloaded = viewModel::onMarkDeloaded
                    )
                }

                NavTile(
                    label = "Gym",
                    word = "TRAIN",
                    primary = true,
                    onClick = onGoToGym,
                    heightDp = 160,
                    modifier = Modifier.fillMaxWidth()
                )

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    NavTile(
                        label = "Cardio",
                        word = "MOVE",
                        primary = false,
                        onClick = onGoToCardio,
                        heightDp = 110,
                        modifier = Modifier.weight(1f)
                    )
                    NavTile(
                        label = "Trophies",
                        word = "EARN",
                        primary = false,
                        onClick = onGoToTrophies,
                        heightDp = 110,
                        modifier = Modifier.weight(1f)
                    )
                }
            }
        }
    }
}
