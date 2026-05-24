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
import androidx.room.Room
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.program.Program
import kotlinx.coroutines.runBlocking

/**
 * Home screen widget showing next planned workout day + main exercises (#146).
 * Uses Glance API. Data is fetched synchronously on update.
 */
class ForgeWidget : GlanceAppWidget() {

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        // Read next session info from DB (widgets don't participate in Hilt injection)
        val db = Room.databaseBuilder(context, ForgeDatabase::class.java, "forge.db")
            .fallbackToDestructiveMigration(dropAllTables = true).build()
        val nextDayKey = runBlocking { db.sessionDao().allFinished().lastOrNull()?.dayKey }
        val nextDayPlan = nextDayKey?.let { k ->
            val rotation = listOf(Program.UPPER_A, Program.LOWER_A, Program.UPPER_B, Program.LOWER_B)
            val idx = rotation.indexOf(k)
            if (idx < 0) null
            else Program.days.firstOrNull { it.key == rotation[(idx + 1) % rotation.size] }
        } ?: Program.days.firstOrNull()

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
