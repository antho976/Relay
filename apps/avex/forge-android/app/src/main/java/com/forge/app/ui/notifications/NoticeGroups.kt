package com.forge.app.ui.notifications

import com.forge.app.data.repo.AppNotice
import com.forge.app.data.repo.NoticeAction
import com.forge.app.data.repo.NoticeKind

/**
 * The feed's three groups (2026-10-21 redesign, "pg-notif nf"), by what each row asks of you:
 * decisions first, news next, setup last.
 */
internal enum class NoticeGroup(val label: String) {
    NEEDS_YOU("Needs you"),
    NEW("New"),
    SET_UP("Set up"),
}

internal fun groupOf(kind: NoticeKind): NoticeGroup = when (kind) {
    NoticeKind.ACTIVE_SESSION, NoticeKind.CHECK_IN, NoticeKind.COACH_BRIEF -> NoticeGroup.NEEDS_YOU
    NoticeKind.MILESTONE, NoticeKind.ACADEMY, NoticeKind.RESULT -> NoticeGroup.NEW
    NoticeKind.SETUP -> NoticeGroup.SET_UP
}

/** The notices in their groups, groups in order and empty ones left out; feed order kept within each. */
internal fun groupNotices(notices: List<AppNotice>): List<Pair<NoticeGroup, List<AppNotice>>> =
    NoticeGroup.entries.mapNotNull { g -> notices.filter { groupOf(it.kind) == g }.takeIf { it.isNotEmpty() }?.let { g to it } }

/**
 * The verb a row carries as its button, when acting on it is a decision made right here (check in,
 * allow, connect, resume). Null for a row that goes somewhere (it carries a chevron) or does nothing.
 */
internal fun actionVerb(action: NoticeAction): String? = when (action) {
    NoticeAction.OpenCheckin -> "Check in"
    is NoticeAction.ResumeSession -> "Resume"
    NoticeAction.EnableNotifications -> "Allow"
    NoticeAction.ConnectWearable -> "Connect"
    else -> null
}

/** The page's mono subtitle: "6 waiting · swipe a row to clear it", honest at zero. */
internal fun noticesMetaLine(notices: List<AppNotice>): String {
    val base = "${notices.size} waiting"
    return if (notices.any { it.dismissible }) "$base · swipe a row to clear it" else base
}
