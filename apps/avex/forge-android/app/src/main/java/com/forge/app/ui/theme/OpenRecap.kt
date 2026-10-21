package com.forge.app.ui.theme

import androidx.compose.ui.unit.sp

/** The Recap page's rungs (2026-10-21 redesign, "pg-recap"). */
object RecapType {
    /** The period's name under the tabs ("October", "2026"). */
    val period = OpenType.figureL.copy(fontSize = 40.sp, lineHeight = 42.sp)

    /** One of the four figures ("12", "248"). */
    val figure = OpenType.figureL.copy(fontSize = 30.sp, lineHeight = 32.sp)

    /** A figure's unit or suffix ("k", " wk"). */
    val figureUnit = OpenType.figureL.copy(fontSize = 16.sp)
}
