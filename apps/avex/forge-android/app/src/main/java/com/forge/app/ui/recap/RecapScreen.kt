package com.forge.app.ui.recap

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.formatVolumeCompact
import com.forge.app.ui.common.EditorialFigure
import com.forge.app.ui.common.EditorialHeader
import com.forge.app.ui.common.ForgeRowGroup
import com.forge.app.ui.common.ForgeShimmerHost
import com.forge.app.ui.common.ForgeTopBar
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.forgeShimmer
import com.forge.app.ui.theme.LocalForgeSettings

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RecapScreen(
    onBack: () -> Unit,
    viewModel: RecapViewModel = hiltViewModel()
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val weightUnit = LocalForgeSettings.current.weightUnit
    val cs = MaterialTheme.colorScheme
    val muted = cs.onSurfaceVariant
    val accent = cs.primary
    val onBg = cs.onBackground

    Scaffold(
        topBar = {
            // §4.6: chrome only, never the screen's name; the serif hero below carries it.
            ForgeTopBar(onBack = onBack)
        },
        containerColor = Color.Transparent
    ) { inner ->
        if (state.isLoading) {
            // A skeleton that mirrors the two recap sections anchors the layout, so content swaps in
            // without a jump — beats a generic spinner that teaches the user nothing (#404).
            RecapSkeleton(Modifier.fillMaxSize().padding(inner).padding(horizontal = 24.dp, vertical = 8.dp))
            return@Scaffold
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(inner)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(0.dp)
        ) {
            // A tiny serif hero names the screen in content (§2).
            Text("Recap", style = MaterialTheme.typography.headlineSmall, color = onBg)
            Spacer(Modifier.height(20.dp))
            // Monthly recap section (#32)
            state.monthRecap?.let { recap ->
                RecapSection(
                    title = "THIS MONTH · ${recap.month.month.name.lowercase().replaceFirstChar { it.uppercase() }} ${recap.month.year}",
                    muted = muted, accent = accent
                ) {
                    RecapFiguresRow(
                        figures = listOf(
                            Pair("${recap.sessionCount}", "workouts"),
                            Pair(formatRecapVolume(recap.totalVolumeLb, weightUnit), "total volume"),
                            Pair("${recap.totalPrs}", "prs"),
                            Pair("${recap.totalSets}", "sets")
                        ),
                        onBg = onBg, muted = muted, accent = accent
                    )
                    RecapRows(
                        listOfNotNull(
                            recap.topExercise?.let { "Most trained" to it },
                            ("Avg session" to "${recap.avgDurationMin} min").takeIf { recap.avgDurationMin > 0 },
                            recap.bestDayName?.let { "Best PR day" to it }
                        ),
                        onBg, muted
                    )
                }
            } ?: RecapSection(title = "THIS MONTH", muted = muted, accent = accent) {
                // §12: an empty period shows the same figures at honest zeros, never a hidden section.
                RecapFiguresRow(
                    figures = listOf(
                        Pair("0", "workouts"),
                        Pair(formatRecapVolume(0.0, weightUnit), "total volume"),
                        Pair("0", "prs"),
                        Pair("0", "sets")
                    ),
                    onBg = onBg, muted = muted, accent = accent
                )
            }

            Spacer(Modifier.height(28.dp))

            // Year-over-year recap section (#33)
            state.yearRecap?.let { recap ->
                RecapSection(
                    title = "${recap.year} IN REVIEW",
                    muted = muted, accent = accent
                ) {
                    RecapFiguresRow(
                        figures = listOf(
                            Pair("${recap.sessionCount}", "workouts"),
                            Pair(formatRecapVolume(recap.totalVolumeLb, weightUnit), "total volume"),
                            Pair("${recap.totalPrs}", "prs"),
                            Pair("${recap.longestStreak}d", "streak")
                        ),
                        onBg = onBg, muted = muted, accent = accent
                    )
                    RecapRows(
                        listOfNotNull(
                            ("Avg weekly volume" to formatRecapVolume(recap.avgWeeklyVolume, weightUnit))
                                .takeIf { recap.avgWeeklyVolume > 0 },
                            recap.topExercise?.let { "Most trained exercise" to it },
                            recap.bestMonthName?.let { "Best month" to it }
                        ),
                        onBg, muted
                    )
                }
            } ?: RecapSection(title = "THIS YEAR", muted = muted, accent = accent) {
                // §12: honest zeros, same vocabulary as the populated section.
                RecapFiguresRow(
                    figures = listOf(
                        Pair("0", "workouts"),
                        Pair(formatRecapVolume(0.0, weightUnit), "total volume"),
                        Pair("0", "prs"),
                        Pair("0d", "streak")
                    ),
                    onBg = onBg, muted = muted, accent = accent
                )
            }

            Spacer(Modifier.height(16.dp))
        }
    }
}

/** Loading placeholder mirroring the two recap sections (label · big stats · rows) — uses the shared
 *  [forgeShimmer] so the populated content swaps in without a layout jump (#404). */
@Composable
private fun RecapSkeleton(modifier: Modifier = Modifier) = ForgeShimmerHost {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(16.dp)) {
        repeat(2) {
            Column(
                Modifier.fillMaxWidth().padding(vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp)
            ) {
                Box(Modifier.width(180.dp).height(12.dp).clip(RoundedCornerShape(50)).forgeShimmer())
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    repeat(4) {
                        Box(Modifier.width(56.dp).height(40.dp).clip(RoundedCornerShape(8.dp)).forgeShimmer())
                    }
                }
                repeat(3) {
                    Box(Modifier.fillMaxWidth().height(12.dp).clip(RoundedCornerShape(50)).forgeShimmer())
                }
            }
        }
    }
}

/** Open editorial section: small-caps label → content, directly on the page background — air alone
 *  separates sections (§1: no hairline strips). */
@Composable
private fun RecapSection(
    title: String,
    muted: Color,
    accent: Color,
    content: @Composable () -> Unit
) {
    EditorialHeader(label = title, muted = muted, accent = accent)
    Spacer(Modifier.height(16.dp))
    content()
}

/** Four open serif figures in a row — the headline stats for a recap period. */
@Composable
private fun RecapFiguresRow(
    figures: List<Pair<String, String>>,
    onBg: Color,
    muted: Color,
    accent: Color
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        figures.forEach { (value, label) ->
            EditorialFigure(
                value = value,
                label = label,
                onBg = onBg,
                muted = muted,
                accent = accent,
                // Equal columns so a wide volume figure can't crowd the row off-screen.
                modifier = Modifier.weight(1f)
            )
        }
    }
    Spacer(Modifier.height(14.dp))
}

/** Volume for the recap sections: "12.5k lb" for big numbers, the exact value under 1000 lb (no "0k"). */
private fun formatRecapVolume(lb: Double, weightUnit: WeightUnit): String = formatVolumeCompact(lb, weightUnit)

/** The period's label and value readings as one group of filled rows, 2dp seams between them. */
@Composable
private fun RecapRows(rows: List<Pair<String, String>>, onBg: Color, muted: Color) {
    if (rows.isEmpty()) return
    Spacer(Modifier.height(6.dp))
    ForgeRowGroup(
        *rows.map { (label, value) ->
            @Composable { RecapRow(label, value, onBg, muted) }
        }.toTypedArray()
    )
}

@Composable
private fun RecapRow(
    label: String,
    value: String,
    onBg: Color,
    muted: Color
) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = ROW_H, vertical = 14.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, color = muted, modifier = Modifier.weight(1f))
        Text(
            value,
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.SemiBold,
            color = onBg,
            textAlign = TextAlign.End,
            modifier = Modifier.weight(1f, fill = false)
        )
    }
}
