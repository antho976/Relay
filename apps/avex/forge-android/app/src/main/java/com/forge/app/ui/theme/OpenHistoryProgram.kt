package com.forge.app.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

/**
 * The rungs and greys the History, Session detail and Program screens draw with that [OpenType] and
 * [OpenColors] do not already hold (2026-10-21 redesign, Relay designs `history_*` and `program_*`).
 * Kept apart from the shared kit so those screens can be tuned without touching it.
 */
object HistoryType {
    /** A lift row's serif figure on the session page ("195 lb", "4,220"). */
    val liftFigure = OpenType.figureXL.copy(fontSize = 20.sp, lineHeight = 22.sp)

    /** The " × " between a set's weight and reps, at half the figure's size. */
    val liftFigureTimes = OpenType.figureXL.copy(fontSize = 14.sp)

    /** The session page's summary figures ("19.8k", "18"). */
    val summaryFigure = OpenType.figureXL.copy(fontSize = 26.sp, lineHeight = 28.sp)

    /** The ↑ beside a summary figure that moved. */
    val summaryArrow = OpenType.monoS.copy(fontSize = 12.sp, letterSpacing = 0.sp)

    /** Heart rate's average ("118") and its unit (" bpm"). */
    val heartFigure = OpenType.figureXL.copy(fontSize = 30.sp, lineHeight = 32.sp)
    val heartUnit = OpenType.figureXL.copy(fontSize = 15.sp)

    /** The date line under a session's name, and the journal under that: serif italic. */
    val dateLine = OpenType.body.copy(fontFamily = AvexFonts.Serif, fontStyle = FontStyle.Italic, fontSize = 15.sp, lineHeight = 21.sp)
    val journal = OpenType.body.copy(fontFamily = AvexFonts.Serif, fontStyle = FontStyle.Italic, fontSize = 14.sp, lineHeight = 20.sp)

    /** A filter chip's or a tight choice's label (All, Gym, Push). */
    val choice = OpenType.buttonS.copy(fontWeight = FontWeight.SemiBold)

    /** A weekday choice ("M", "Any"). */
    val weekday = OpenType.monoM.copy(letterSpacing = 0.sp)
    val weekdayOn = weekday.copy(fontWeight = FontWeight.Bold)

    /** The coach's change tag on a lift ("↑ 1 REP"). */
    val changeTag = OpenType.monoS.copy(fontSize = 9.sp, lineHeight = 10.sp, letterSpacing = 1.sp, fontWeight = FontWeight.Bold)

    /** A planned lift's sets × reps on the Program page ("4 × 5"). */
    val planFigure = OpenType.figureS

    /** A lift row's title on the Program page. */
    val planLift = OpenType.rowTitleL
}

/** The greys these screens add to [OpenColors]. */
object HistoryColors {
    /** A closed lift's name on the session page: a step under ink so the open one leads. */
    val NameQuiet = Color(0xFFD9D2C8)

    /** The track behind a day's bar on the Program page. */
    val DayTrack = Color(0xFF1E1A16)

    /** Remove day: a muted red line and label, not the filled accent. */
    val Remove = Color(0xFFE57373)
    val RemoveEdge = Color(0x99BF4040)
}
