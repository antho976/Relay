package com.forge.app.data.repo

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationManagerCompat
import com.forge.app.core.time.TimeSignals
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.notify.Milestones
import com.forge.app.program.Program
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withContext
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.temporal.IsoFields
import java.util.Locale
import javax.inject.Inject
import javax.inject.Singleton

/** What a notice does when its row is tapped. [None] renders the row passive (DESIGN §2③). */
sealed interface NoticeAction {
    data class ResumeSession(val dayKey: String) : NoticeAction
    data object OpenCheckin : NoticeAction
    data object OpenCoachBrief : NoticeAction
    data object ConnectWearable : NoticeAction

    /** Opens one Academy lesson. The row exists because its coach moment fired, not on a schedule. */
    data class OpenLesson(val lessonId: String) : NoticeAction

    /** Opens the OS app-notification settings. Not app navigation, so the screen handles it. */
    data object EnableNotifications : NoticeAction
    data object None : NoticeAction
}

/**
 * A switchable category of notice. Each is one toggle on Settings → Notifications, so the user can
 * turn a whole kind off rather than clearing its rows over and over. [key] is the stored id and must
 * never change; [label] and [explainer] are that toggle's row.
 */
enum class NoticeKind(val key: String, val label: String, val explainer: String) {
    // Each explainer says what the ROW will offer, never what its own label already said — "a workout
    // you started but haven't finished" is a definition of "unfinished workout", not information.
    //
    // DECLARATION ORDER IS FEED RANK (DESIGN §4.8, lead with the live): what you are mid-way
    // through, then what waits on a decision, then what happened, then housekeeping, then invites.
    // [key] is the stored id and must never change, but reordering these is free and is how a new
    // kind picks its place in the list.
    ACTIVE_SESSION("active_session", "Unfinished workouts", "Resume where you stopped"),
    CHECK_IN("check_in", "Daily check-ins", "Sleep, soreness, stress, and drive"),
    COACH_BRIEF("coach_brief", "Coach briefs", "Each week's read, when it's new"),
    MILESTONE("milestone", "Milestones", "100 workouts, your first full month"),

    /**
     * A lesson the coach just opened for you.
     *
     * This kind is the reason the Academy never interrupts. A new lesson is knowledge that became
     * relevant, not a decision waiting on you, so it waits behind the bell until you go looking.
     * The toggle exists for readers who would rather find lessons themselves.
     */
    ACADEMY("academy", "New lessons", "When the coach opens one for you"),
    RESULT("result", "Imports and backups", "What an import or restore brought in"),
    SETUP("setup", "Setup invites", "Connect a watch, turn on alerts"),
}

/**
 * Which glyph leads a row. A symbol rather than an `ImageVector` so the feed stays free of UI types;
 * `ui/notifications/NoticeIcons.kt` owns the drawing.
 */
enum class NoticeGlyph { SESSION, COACH, MILESTONE, IMPORT, BACKUP, HOUSEKEEPING, BELL, WATCH, ACADEMY }

/**
 * One row of the notifications feed. [eyebrow] is the mono kind label, [title] the line itself and
 * [detail] its one supporting reading — never a second sentence of narration (§4.3).
 */
data class AppNotice(
    val id: String,
    val kind: NoticeKind,
    val glyph: NoticeGlyph,
    val eyebrow: String,
    val title: String,
    val detail: String? = null,
    val action: NoticeAction = NoticeAction.None,
    /** False for a notice the user can't clear because it clears itself (a live session ends). */
    val dismissible: Boolean = true,
)

/**
 * The app's single notifications feed — everything that used to interrupt a page as a banner.
 *
 * Home carried four strips (resume, coach brief, milestone, orphan notice) and Cardio a fifth (the
 * connect-a-watch invite); each pushed the page's real content down and each had its own dismissal
 * rule. They are one list here, and the pages are left to say one thing.
 *
 * Sources split two ways. Most are already observable (the active session, the prefs), so they flow
 * straight through. Two are not: the weekly coach pass is a suspend call and Health Connect exposes
 * no permission-change observable, so both are polled by [refresh] at the moments they can change
 * (app open, resume) and cached in a state flow.
 */
@Singleton
class NotificationFeed @Inject constructor(
    @ApplicationContext private val context: Context,
    private val settings: SettingsRepository,
    private val workoutRepo: WorkoutRepository,
    private val checkinRepo: CheckinRepository,
    private val coachRepo: CoachRepository,
    private val healthConnect: HealthConnectManager,
    private val academyRepo: AcademyRepository,
    private val timeSignals: TimeSignals,
) {
    private val coachBrief = MutableStateFlow<CoachBanner?>(null)

    /** Whether a watch/ring already feeds Health Connect. Starts true so the invite can't flash on
     *  screen for a connected user in the moment before the first grant read lands. */
    private val wearableConnected = MutableStateFlow(true)

    /** Whether the OS lets the app post notifications. Starts true for the same no-flash reason. */
    private val notificationsAllowed = MutableStateFlow(true)

    /**
     * Re-read the sources with no observable of their own. Idempotent and cheap: the weekly pass is
     * keyed by ISO week, and the grant reads are local IPC.
     */
    suspend fun refresh() {
        // Fire whatever Academy moments the current data implies, BEFORE anything reads the ledger.
        // Without this a lesson would only unlock on a visit to the Academy tab, so the one surface
        // meant to tell you a lesson exists could never be the thing that told you first. Every
        // unlock is idempotent, so calling it on each resume is free.
        runCatching { academyRepo.syncCoachMoments() }
        coachBrief.value = runCatching { coachRepo.pendingBanner() }.getOrNull()
        wearableConnected.value = runCatching {
            healthConnect.canReadSteps() || healthConnect.canReadExercise()
        }.getOrDefault(false)
        notificationsAllowed.value = osNotificationsEnabled(context)
    }

    /**
     * Lessons whose moment has fired but that haven't been read, minus any the user cleared.
     *
     * Derived rather than stored: a lesson row exists exactly while the lesson is unlocked and
     * unopened, so opening one clears its row with no extra bookkeeping. Only the DISMISSAL needs a
     * record of its own, and it lives in prefs rather than the ledger — the ledger is append-only
     * and records what happened, and writing a fake "opened" event to silence a row would corrupt
     * the read history the Academy derives everything from.
     */
    private val academyNotices: Flow<List<AppNotice>> = combine(
        academyRepo.observeStates(),
        settings.dismissedLessonNotices,
    ) { states, dismissed ->
        states
            .filter { it.isNew && it.lesson.id !in dismissed }
            // Newest unlock first, so a burst of moments reads in the order they happened.
            .sortedByDescending { it.unlockedAtMs ?: 0L }
            .map { state ->
                AppNotice(
                    id = "$PREFIX_LESSON${state.lesson.id}",
                    kind = NoticeKind.ACADEMY,
                    glyph = NoticeGlyph.ACADEMY,
                    eyebrow = "NEW LESSON",
                    title = state.lesson.title,
                    detail = state.lesson.summary,
                    action = NoticeAction.OpenLesson(state.lesson.id),
                )
            }
    }

    /** Pending until today's sheet is saved. Rebinds at midnight and after a timezone change, then
     * Room removes the notice as soon as the write lands. Legacy skipped rows do not resolve it. */
    @OptIn(ExperimentalCoroutinesApi::class)
    private val checkinNotice: Flow<AppNotice?> = timeSignals.dayStarts().flatMapLatest {
        val dateKey = checkinRepo.todayKey()
        checkinRepo.observeToday().map { entry ->
            if (entry?.skipped == false) null else AppNotice(
                id = "$PREFIX_CHECKIN$dateKey",
                kind = NoticeKind.CHECK_IN,
                glyph = NoticeGlyph.COACH,
                eyebrow = "TODAY",
                title = "How are you feeling today?",
                detail = "Sleep, soreness, stress, drive, and optional weight.",
                action = NoticeAction.OpenCheckin,
                // Saving resolves it. Clear all should not silently skip today's check-in.
                dismissible = false,
            )
        }
    }

    /**
     * Every live notice, ranked by DESIGN §4.8 (lead with the live): what you're mid-way through,
     * then what's waiting on a decision, then what happened, then housekeeping, then the invite.
     */
    val notices: Flow<List<AppNotice>> = combine(
        workoutRepo.observeActiveSession(),
        coachBrief,
        settings.unreadMilestones,
        settings.systemNotices,
        settings.weightUnit,
    ) { active, brief, milestones, systemNotices, weightUnit ->
        buildList {
            // A session whose day was dropped by a program regenerate can't be resumed — the same
            // guard Home's CTA uses, so the feed never offers a day that no longer exists.
            active?.dayKey?.takeIf { it in Program.dayKeys }?.let { dayKey ->
                add(
                    AppNotice(
                        id = ID_ACTIVE_SESSION,
                        kind = NoticeKind.ACTIVE_SESSION,
                        glyph = NoticeGlyph.SESSION,
                        eyebrow = "IN PROGRESS",
                        title = Program.days.firstOrNull { it.key == dayKey }?.defaultName ?: "Workout",
                        detail = "Picks up where you stopped.",
                        action = NoticeAction.ResumeSession(dayKey),
                        // Ends by being finished or abandoned, not by being swept out of a list.
                        dismissible = false,
                    )
                )
            }
            brief?.let { banner ->
                add(
                    AppNotice(
                        id = "$PREFIX_COACH${banner.weekId}",
                        kind = NoticeKind.COACH_BRIEF,
                        glyph = NoticeGlyph.COACH,
                        eyebrow = "COACH BRIEF",
                        title = banner.text.removePrefix("Coach · ").replaceFirstChar(Char::uppercase),
                        detail = weekLabel(banner.weekId),
                        action = NoticeAction.OpenCoachBrief,
                    )
                )
            }
            milestones.sorted().forEach { id ->
                Milestones.messageFor(id, weightUnit)?.let { message ->
                    add(
                        AppNotice(
                            id = "$PREFIX_MILESTONE$id",
                            kind = NoticeKind.MILESTONE,
                            glyph = NoticeGlyph.MILESTONE,
                            eyebrow = "MILESTONE",
                            title = message,
                        )
                    )
                }
            }
            systemNotices.forEach { notice ->
                add(
                    AppNotice(
                        id = "$PREFIX_SYSTEM${notice.id}",
                        kind = NoticeKind.RESULT,
                        glyph = glyphFor(notice.id),
                        eyebrow = eyebrowFor(notice.id),
                        title = notice.text,
                    )
                )
            }
        }
    }.combine(
        // Both invites carry two gates each, so they're folded together rather than widening the
        // combine above: hidden once dismissed for good, and hidden once the thing is actually set up.
        combine(
            settings.cardioWearableHintDismissed, wearableConnected,
            settings.notifPermAsked, notificationsAllowed,
        ) { wearableDismissed, wearableOn, notifAsked, notifOn ->
            (wearableDismissed || wearableOn) to (notifAsked || notifOn)
        }
    ) { rows, (hideWearable, hideNotifications) ->
        buildList {
            addAll(rows)
            if (!hideNotifications) add(
                AppNotice(
                    id = ID_NOTIF_PERMISSION,
                    kind = NoticeKind.SETUP,
                    glyph = NoticeGlyph.BELL,
                    eyebrow = "CONNECT",
                    title = "Turn on notifications",
                    detail = "Training nudges, your weekly recap, and a ping when a rest timer ends.",
                    action = NoticeAction.EnableNotifications,
                )
            )
            if (!hideWearable) add(
                AppNotice(
                    id = ID_WEARABLE,
                    kind = NoticeKind.SETUP,
                    glyph = NoticeGlyph.WATCH,
                    eyebrow = "CONNECT",
                    title = "Connect a watch or ring",
                    detail = "Steps by the hour and your route on each session.",
                    action = NoticeAction.ConnectWearable,
                )
            )
        }
    }.combine(checkinNotice) { rows, checkin ->
        if (checkin == null) rows else rows + checkin
    }.combine(academyNotices) { rows, academy ->
        // Sorted by kind rather than appended, so a new kind's place in the feed is decided by its
        // position in the enum (§4.8) instead of by where its builder happened to be called. The
        // sort is stable and the existing rows were already in kind order, so nothing else moves.
        (rows + academy).sortedBy { it.kind.ordinal }
    }.combine(settings.disabledNoticeKinds) { rows, disabled ->
        // A kind switched off on Settings → Notifications never reaches the feed. Applied last so a
        // notice is still QUEUED while its kind is off — switching it back on brings the row back
        // rather than having silently dropped it.
        if (disabled.isEmpty()) rows else rows.filterNot { it.kind.key in disabled }
    }

    /** How many notices are waiting — the bell's unread count. */
    val unreadCount: Flow<Int> = notices.map { it.size }

    /** How many of those are lessons — the Academy tab's badge. Reads from the same list as the
     *  bell, so a kind switched off in Settings drops out of both at once. */
    val academyUnreadCount: Flow<Int> = notices.map { rows -> rows.count { it.kind == NoticeKind.ACADEMY } }

    /** Notices whose small arrival banner has not played yet. The feed row and banner have separate
     * lifetimes so an unanswered check-in or unread lesson does not pop up again on every launch. */
    val pendingAnnouncements: Flow<List<AppNotice>> = combine(
        academyNotices,
        checkinNotice,
        settings.announcedLessonNotices,
        settings.announcedCheckinDates,
        settings.disabledNoticeKinds,
    ) { academy, checkin, announcedLessons, announcedCheckins, disabled ->
        buildList {
            if (NoticeKind.CHECK_IN.key !in disabled && checkin != null &&
                checkin.id.removePrefix(PREFIX_CHECKIN) !in announcedCheckins
            ) add(checkin)
            if (NoticeKind.ACADEMY.key !in disabled) {
                addAll(academy.filter { it.id.removePrefix(PREFIX_LESSON) !in announcedLessons })
            }
        }
    }

    /** Mark arrival banners as played. One-way: an announcement happens once, ever. */
    suspend fun markAnnounced(noticeIds: List<String>) {
        settings.markLessonNoticesAnnounced(
            noticeIds.filter { it.startsWith(PREFIX_LESSON) }
                .map { it.removePrefix(PREFIX_LESSON) }
                .toSet()
        )
        settings.markCheckinDatesAnnounced(
            noticeIds.filter { it.startsWith(PREFIX_CHECKIN) }
                .map { it.removePrefix(PREFIX_CHECKIN) }
                .toSet()
        )
    }

    /**
     * Clear one notice and hand back the operation that puts it back, so the caller can offer an Undo
     * (§12). Each source owns its own inverse: the coach brief restores the previously-seen week id,
     * a milestone re-queues (it can still never re-FIRE), and the invite un-dismisses.
     */
    suspend fun dismiss(id: String): suspend () -> Unit = when {
        id.startsWith(PREFIX_COACH) -> {
            val previous = settings.lastSeenCoachWeekId.first()
            val restored = coachBrief.value
            coachRepo.markSeen(id.removePrefix(PREFIX_COACH))
            coachBrief.value = null
            ({
                settings.setLastSeenCoachWeekId(previous)
                coachBrief.value = restored
            })
        }

        id.startsWith(PREFIX_MILESTONE) -> {
            val milestoneId = id.removePrefix(PREFIX_MILESTONE)
            settings.setMilestoneUnread(milestoneId, unread = false)
            ({ settings.setMilestoneUnread(milestoneId, unread = true) })
        }

        id.startsWith(PREFIX_LESSON) -> {
            val lessonId = id.removePrefix(PREFIX_LESSON)
            settings.setLessonNoticeDismissed(lessonId, dismissed = true)
            ({ settings.setLessonNoticeDismissed(lessonId, dismissed = false) })
        }

        id.startsWith(PREFIX_SYSTEM) -> {
            val noticeId = id.removePrefix(PREFIX_SYSTEM)
            val previous = settings.systemNotices.first().firstOrNull { it.id == noticeId }
            settings.removeSystemNotice(noticeId)
            ({ previous?.let { settings.addSystemNotice(it.id, it.text) } })
        }

        id == ID_WEARABLE -> {
            settings.setCardioWearableHintDismissed(true)
            ({ settings.setCardioWearableHintDismissed(false) })
        }

        id == ID_NOTIF_PERMISSION -> {
            settings.setNotifPermAsked(true)
            ({ settings.setNotifPermAsked(false) })
        }

        else -> ({})
    }

    /**
     * Clear everything clearable in one go, returning the single operation that restores them all.
     *
     * All or nothing (M-27): if a later dismissal fails, the ones already made are put back before
     * the failure propagates, so the caller never publishes an Undo that would restore only part of
     * the list, and never leaves a half-cleared feed with no Undo at all. The rollback runs shielded
     * because the failure that got here may itself be the caller's cancellation.
     */
    suspend fun dismissAll(): suspend () -> Unit {
        val undos = mutableListOf<suspend () -> Unit>()
        try {
            notices.first().filter { it.dismissible }.forEach { undos += dismiss(it.id) }
        } catch (e: Exception) {
            withContext(NonCancellable) {
                undos.asReversed().forEach { undo -> runCatching { undo() } }
            }
            throw e
        }
        return { undos.forEach { it() } }
    }

    /** Opening the brief counts as seeing it, same as clearing the row. */
    suspend fun markCoachBriefSeen() {
        coachBrief.value?.let { coachRepo.markSeen(it.weekId) }
        coachBrief.value = null
    }

    companion object {
        /** [SettingsRepository.addSystemNotice] ids — the results that used to be an OK dialog. */
        const val NOTICE_ORPHAN_SESSION = "orphan"
        const val NOTICE_IMPORT = "import"
        const val NOTICE_RESTORE = "restore"

        private const val ID_ACTIVE_SESSION = "session.active"
        private const val ID_WEARABLE = "connect.wearable"
        private const val ID_NOTIF_PERMISSION = "connect.notifications"
        private const val PREFIX_COACH = "coach."
        private const val PREFIX_MILESTONE = "milestone."
        private const val PREFIX_SYSTEM = "system."
        private const val PREFIX_LESSON = "lesson."
        private const val PREFIX_CHECKIN = "checkin."

        private fun eyebrowFor(noticeId: String): String = when (noticeId) {
            NOTICE_IMPORT -> "IMPORT"
            NOTICE_RESTORE -> "BACKUP"
            else -> "HOUSEKEEPING"
        }

        private fun glyphFor(noticeId: String): NoticeGlyph = when (noticeId) {
            NOTICE_IMPORT -> NoticeGlyph.IMPORT
            NOTICE_RESTORE -> NoticeGlyph.BACKUP
            else -> NoticeGlyph.HOUSEKEEPING
        }

        private val WEEK_DAY_FORMAT: DateTimeFormatter = DateTimeFormatter.ofPattern("d MMM", Locale.getDefault())

        /**
         * Whether the OS will actually deliver this app's notifications.
         *
         * Every version below 13 used to be reported as allowed outright, on the grounds that the
         * runtime permission did not exist yet. But the user can block an app's notifications in
         * OS settings on any version, and on Android 8 to 12 that block hid the feed's only
         * recovery row (M-28). [NotificationManagerCompat.areNotificationsEnabled] answers the
         * real question on every supported API; the runtime permission is checked alongside it on
         * 13+, where a missing grant is the same outcome by a different route. The user's own
         * "don't remind me" choice is a separate preference and stays out of this read.
         */
        internal fun osNotificationsEnabled(context: Context): Boolean {
            val permitted = Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
                context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) ==
                PackageManager.PERMISSION_GRANTED
            return permitted && NotificationManagerCompat.from(context).areNotificationsEnabled()
        }

        /**
         * "2026-W27" reads as "Week of 29 Jun" (§11: machine identifiers never render). An id that
         * doesn't parse falls back to nothing rather than leaking the key into the UI.
         */
        private fun weekLabel(weekId: String): String? {
            val (year, week) = weekId.split("-W").takeIf { it.size == 2 } ?: return null
            val y = year.toIntOrNull() ?: return null
            val w = week.toIntOrNull() ?: return null
            return runCatching {
                LocalDate.of(y, 1, 4)
                    .with(IsoFields.WEEK_BASED_YEAR, y.toLong())
                    .with(IsoFields.WEEK_OF_WEEK_BASED_YEAR, w.toLong())
                    .with(java.time.temporal.TemporalAdjusters.previousOrSame(java.time.DayOfWeek.MONDAY))
                    .format(WEEK_DAY_FORMAT)
            }.getOrNull()?.let { "Week of $it" }
        }
    }
}
