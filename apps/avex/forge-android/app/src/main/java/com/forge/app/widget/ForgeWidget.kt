package com.forge.app.widget

import android.content.Context
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.provideContent
import androidx.glance.background
import androidx.glance.layout.Alignment
import androidx.glance.layout.Column
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.padding
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.repo.ProgramRepository
import com.forge.app.program.Program
import dagger.hilt.EntryPoint
import dagger.hilt.InstallIn
import dagger.hilt.android.EntryPointAccessors
import dagger.hilt.components.SingletonComponent

/**
 * Home screen widget showing next planned workout day + main exercises (#146).
 * Uses Glance API. Data is fetched synchronously on update.
 */
class ForgeWidget : GlanceAppWidget() {

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        // Reuse the Hilt singleton DB + program facade via an EntryPoint instead of opening a
        // second Room instance. This removes the destructive-migration data wipe and the leaked
        // connection the old standalone builder caused, and keeps the widget on the same
        // migration policy as the app. ensureLoaded() seeds-if-empty and populates Program.
        val entryPoint = EntryPointAccessors.fromApplication(
            context.applicationContext, WidgetEntryPoint::class.java
        )
        entryPoint.programRepository().ensureLoaded()

        // Next day = the day after the most recently finished session, cycling through the
        // *active* program's days (mirrors StatsRepository.nextUpDayKey — no hard-coded 4-day rotation).
        val lastDayKey = entryPoint.sessionDao().allFinished()
            .maxByOrNull { it.finishedAt ?: it.startedAt }?.dayKey
        val keys = Program.dayKeys
        val nextDayKey = when {
            keys.isEmpty() -> null
            lastDayKey == null -> keys.first()
            // A stale last-day key (day-count changed since) isn't in the current split — start over
            // at the first day rather than letting indexOf(-1) resolve to it.
            else -> keys.indexOf(lastDayKey).let { idx -> if (idx < 0) keys.first() else keys[(idx + 1) % keys.size] }
        }
        val nextDayPlan = nextDayKey?.let { key -> Program.days.firstOrNull { it.key == key } }

        provideContent {
            GlanceTheme {
                Column(
                    modifier = GlanceModifier.fillMaxSize().background(ColorProvider(android.graphics.Color.BLACK))
                        .padding(horizontal = 12, vertical = 8),
                    verticalAlignment = Alignment.Top
                ) {
                    Text(
                        "FORGE",
                        style = TextStyle(
                            fontWeight = FontWeight.Bold,
                            color = ColorProvider(android.graphics.Color.parseColor("#6650A4"))
                        )
                    )
                    if (nextDayPlan != null) {
                        Text(
                            nextDayPlan.defaultName.uppercase(),
                            style = TextStyle(
                                fontWeight = FontWeight.Bold,
                                color = ColorProvider(android.graphics.Color.WHITE)
                            )
                        )
                        Text(
                            "${nextDayPlan.exercises.size} exercises · ${nextDayPlan.word}",
                            style = TextStyle(color = ColorProvider(android.graphics.Color.LTGRAY))
                        )
                        nextDayPlan.exercises.take(3).forEach { ex ->
                            Text(
                                "· ${ex.name}",
                                style = TextStyle(color = ColorProvider(android.graphics.Color.LTGRAY))
                            )
                        }
                    } else {
                        Text("No session data yet", style = TextStyle(color = ColorProvider(android.graphics.Color.LTGRAY)))
                    }
                }
            }
        }
    }
}

class ForgeWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = ForgeWidget()
}

/**
 * Lets the widget (which isn't a Hilt-injected component) reach the app's singleton DB + program
 * repository, so it shares the one migration-aware [ForgeDatabase] instance the app owns.
 */
@EntryPoint
@InstallIn(SingletonComponent::class)
interface WidgetEntryPoint {
    fun sessionDao(): SessionDao
    fun programRepository(): ProgramRepository
}
