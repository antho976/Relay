package com.forge.app.ui.theme

import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

/** The Notifications feed and Check-in sheet's rungs (2026-10-21 redesign, "pg-notif"). */
object NoticeType {
    /** "Notifications": a long word, so a page title one step under [OpenType.title]. */
    val pageTitle = OpenType.title.copy(fontSize = 48.sp, lineHeight = 52.sp, letterSpacing = (-1.2).sp)

    /** A row's inline action ("Check in", "Allow"): 13 semibold in a 32dp outline. */
    val action = OpenType.bodyS.copy(fontWeight = FontWeight.SemiBold, lineHeight = 16.sp)

    /** The check-in sheet's serif title. */
    val sheetTitle = OpenType.figureMS
}
