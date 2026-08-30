package com.forge.app.widget

import android.content.Context
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.color.ColorProvider
import androidx.glance.action.actionParametersOf
import androidx.glance.action.actionStartActivity
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.updateAll
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
import com.forge.app.MainActivity
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.data.repo.ProgramRepository
import com.forge.app.data.repo.StatsRepository
import com.forge.app.program.Program
import dagger.hilt.EntryPoint
import dagger.hilt.InstallIn
import dagger.hilt.android.EntryPointAccessors
import dagger.hilt.components.SingletonComponent
import kotlinx.coroutines.flow.first
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.forge.app.ui.theme.AccentRed
import com.forge.app.ui.theme.PearlBackground
import com.forge.app.ui.theme.PearlMuted
import com.forge.app.ui.theme.PearlOnBg

/** Intent extra key carrying the next-up dayKey — read by MainActivity to open that day. */
const val EXTRA_START_DAY_KEY = "forge.widget.START_DAY_KEY"

/** Intent extra key set when the widget tap is for an active/in-progress session resume. */
const val EXTRA_RESUME_SESSION = "forge.widget.RESUME_SESSION"

/** The same two keys as Glance ActionParameters — Glance writes these into the launch INTENT, so
 *  MainActivity reads them back with plain `getStringExtra` / `getBooleanExtra`. */
private val startDayKeyParam = androidx.glance.action.ActionParameters.Key<String>(EXTRA_START_DAY_KEY)
private val resumeSessionParam = androidx.glance.action.ActionParameters.Key<Boolean>(EXTRA_RESUME_SESSION)

/**
 * Home screen widget showing next planned workout day + main exercises (#146).
 * Uses Glance API. Data is fetched synchronously on update.
 *
 * Tap behaviour:
 *  - Active session in progress → launches MainActivity with EXTRA_RESUME_SESSION=true,
 *    landing on Overview which will surface the active-session card.
 *  - Next day resolved → launches MainActivity with EXTRA_START_DAY_KEY=<dayKey>.
 *    Currently lands on Overview; a future pass can handle the extra to navigate
 *    directly into the day screen once deep-link plumbing exists in ForgeNavHost.
 *  - No next day → launches MainActivity (Overview / first-workout prompt).
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

        // --- Item 2: active/in-progress session detection ---
        // getActiveSession() returns the unique unfinished session row if one exists.
        // Entirely self-contained in the widget's existing WidgetEntryPoint; no shared-file edits.
        val activeSession = entryPoint.sessionDao().getActiveSession()
        val activeDayPlan = activeSession?.let { s ->
            Program.days.firstOrNull { it.key == s.dayKey }
        }

        // Next day via the shared resolver — calendar-aware in weekday mode, legacy day-after-last
        // otherwise — so the widget agrees with the day list and Overview.
        //
        // Three narrow queries, not `allFinished()`. This used to deserialize EVERY finished session
        // as a full entity and walk the list three times — for a three-year daily user, 900 entities
        // and 900 ZonedDateTimes allocated inside the AppWidgetService update window, to answer
        // "which of these seven days have a dot".
        val settings = entryPoint.settingsRepository()
        val zone = java.time.ZoneId.systemDefault()
        val today = java.time.LocalDate.now(zone)
        val monday = today.with(java.time.DayOfWeek.MONDAY)
        val mondayMs = monday.atStartOfDay(zone).toInstant().toEpochMilli()
        val todayStartMs = today.atStartOfDay(zone).toInstant().toEpochMilli()
        val trainedTodayKeys = entryPoint.sessionDao().finishedDayKeysSince(todayStartMs).toSet()
        val nextDayKey = com.forge.app.domain.schedule.WeeklySchedule.resolveNextUp(
            mode = settings.scheduleMode.first(),
            todayIndex = today.dayOfWeek.value - 1,
            schedule = settings.weeklySchedule.first(),
            dayKeys = Program.dayKeys,
            lastFinishedDayKey = entryPoint.sessionDao().lastFinishedDayKey(),
            trainedTodayKeys = trainedTodayKeys
        )
        val nextDayPlan = nextDayKey?.let { key -> Program.days.firstOrNull { it.key == key } }
        // "Go with the flow" — drives the fallback copy below so the widget doesn't claim a plan exists.
        val freestyleMode = settings.freestyleMode.first()

        // --- Item 1: what the tap should carry ---
        // Active session → resume it; otherwise open the next-up day; otherwise just Overview.
        //
        // Through ActionParameters, which Glance writes into the INTENT. The bundle used to be
        // handed to `actionStartActivity`'s third parameter, which is `activityOptions` — the
        // ActivityOptions slot for PendingIntent.getActivity — not intent extras. (The
        // @OptIn(ExperimentalGlanceApi) this file used to carry was the tell: that annotation is
        // required by exactly that overload.) MainActivity really does read these extras, so the
        // widget's whole tap purpose — open Pull B — silently landed on Overview instead, and
        // "Tap to resume" could never resume anything.
        val tapParameters = when {
            activeSession != null -> actionParametersOf(
                resumeSessionParam to true,
                startDayKeyParam to activeSession.dayKey
            )
            nextDayKey != null -> actionParametersOf(startDayKeyParam to nextDayKey)
            else -> actionParametersOf()
        }

        // Theme-matched colours: honour the user's AMOLED + accent choices (the same Pearl palette the
        // app uses) instead of a hardcoded black + Material purple, so the widget reads as one app.
        val amoled = settings.amoledMode.first()
        // Accent off ⇒ the widget goes monochrome too (near-white highlight), matching the app.
        val accentArgb = if (!settings.accentEnabled.first()) PearlOnBg.toArgb()
            else settings.accentColorHex.first().takeIf { it.isNotBlank() }
                ?.let { runCatching { android.graphics.Color.parseColor(it) }.getOrNull() }
                ?: AccentRed.toArgb()
        val bgArgb = if (amoled) android.graphics.Color.BLACK else PearlBackground.toArgb()
        val onBgArgb = PearlOnBg.toArgb()
        val mutedArgb = PearlMuted.toArgb()

        // This-week dot row from `finished`; streak reuses the app's vacation-aware computation
        // (bridges holidays + the one rest-day grace) so the widget can't contradict the in-app number
        // a hand-rolled walk over finished dates would have ignored vacation bridging.
        val finishedDates = entryPoint.sessionDao().finishedAtsSince(mondayMs)
            .mapTo(HashSet()) { java.time.Instant.ofEpochMilli(it).atZone(zone).toLocalDate() }
        val streak = entryPoint.statsRepository().currentStreakDays()
        // "Has this user ever trained" — a zero-state question, so it counts untracked work too.
        // It used to borrow `lastFinishedDayKey()`, which now answers the different question of what
        // to train NEXT and excludes untracked sessions; sharing one query across both would have
        // made an untracked-only history render as a brand-new install.
        val hasAnyFinished = streak >= 1 || finishedDates.isNotEmpty() ||
            entryPoint.sessionDao().hasAnyFinishedSession()
        val weekDots = (0..6).joinToString(" ") { off ->
            if (monday.plusDays(off.toLong()) in finishedDates) "●" else "○"
        }

        provideContent {
            GlanceTheme {
                Column(
                    modifier = GlanceModifier
                        .fillMaxSize()
                        .background(ColorProvider(Color(bgArgb), Color(bgArgb)))
                        .padding(horizontal = 12.dp, vertical = 8.dp)
                        // Item 1: whole-widget tap target — launches MainActivity with EXTRA_START_DAY_KEY
                        // (the next-up day, or the active session's day). MainActivity reads it and the
                        // nav host opens that day on top of Overview (so Back returns home).
                        .clickable(actionStartActivity(MainActivity::class.java, tapParameters)),
                    verticalAlignment = Alignment.Top
                ) {
                    Text(
                        "AVEX",
                        style = TextStyle(
                            fontWeight = FontWeight.Bold,
                            color = ColorProvider(Color(accentArgb), Color(accentArgb))
                        )
                    )
                    if (activeSession != null) {
                        // --- Item 2: "Resume workout" state ---
                        Text(
                            "WORKOUT IN PROGRESS",
                            style = TextStyle(
                                fontWeight = FontWeight.Bold,
                                color = ColorProvider(Color(accentArgb), Color(accentArgb))
                            )
                        )
                        val label = activeDayPlan?.defaultName?.uppercase() ?: activeSession.dayKey.uppercase()
                        Text(
                            label,
                            style = TextStyle(
                                fontWeight = FontWeight.Bold,
                                color = ColorProvider(Color(onBgArgb), Color(onBgArgb))
                            )
                        )
                        Text(
                            "Tap to resume",
                            style = TextStyle(color = ColorProvider(Color(mutedArgb), Color(mutedArgb)))
                        )
                    } else if (nextDayPlan != null) {
                        Text(
                            nextDayPlan.defaultName.uppercase(),
                            style = TextStyle(
                                fontWeight = FontWeight.Bold,
                                color = ColorProvider(Color(onBgArgb), Color(onBgArgb))
                            )
                        )
                        Text(
                            "${nextDayPlan.exercises.size} exercises · ${nextDayPlan.word}",
                            style = TextStyle(color = ColorProvider(Color(mutedArgb), Color(mutedArgb)))
                        )
                        nextDayPlan.exercises.take(3).forEach { ex ->
                            Text(
                                "· ${ex.name}",
                                style = TextStyle(color = ColorProvider(Color(mutedArgb), Color(mutedArgb)))
                            )
                        }
                    } else {
                        // No active session and no next day — the normal state for freestyle (no plan)
                        // and for a custom user who hasn't built a plan yet, so don't claim "your program
                        // is ready". Branch on the actual mode / program state.
                        val (head, sub) = when {
                            freestyleMode -> "Tap to log a workout" to "No fixed plan — open Avex to log your session."
                            Program.dayKeys.isEmpty() -> "Tap to build your plan" to "Set up your program in Avex to get started."
                            else -> "Tap to start your first workout" to "Your program is ready — open Avex to begin."
                        }
                        Text(
                            head,
                            style = TextStyle(
                                fontWeight = FontWeight.Bold,
                                color = ColorProvider(Color(onBgArgb), Color(onBgArgb))
                            )
                        )
                        Text(sub, style = TextStyle(color = ColorProvider(Color(mutedArgb), Color(mutedArgb))))
                    }
                    // Streak + this-week dots (Cat 21) — once there's any finished session to count.
                    if (hasAnyFinished) {
                        if (streak >= 1) {
                            Text(
                                "$streak-day streak",
                                style = TextStyle(
                                    fontWeight = FontWeight.Bold,
                                    color = ColorProvider(Color(accentArgb), Color(accentArgb))
                                )
                            )
                        }
                        Text(
                            weekDots,
                            style = TextStyle(color = ColorProvider(Color(mutedArgb), Color(mutedArgb)))
                        )
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
 * Refresh every placed widget. Fail-soft — a widget redraw must never take down whatever asked
 * for it (a session finish, a date change).
 *
 * The widget's own `updatePeriodMillis` floor is one hour, and until this existed the only in-app
 * trigger was a program regeneration. So "WORKOUT IN PROGRESS" could sit on the home screen for an
 * hour after the workout ended, and the streak and week dots stayed a day stale past midnight.
 */
suspend fun refreshForgeWidgets(context: Context) {
    runCatching { ForgeWidget().updateAll(context) }
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
    fun settingsRepository(): SettingsRepository
    fun statsRepository(): StatsRepository
}
