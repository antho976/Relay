package com.forge.app.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

/**
 * The rungs the Settings redesign (Relay designs st / sa / sp / sw, the lock screens lk1 to lk5)
 * needs beyond [OpenType]: a page title one step under [OpenType.title] for a long name, the chip and
 * small-button labels at their drawn 600 weight, and the lock screen's caption.
 */
object SettingsType {
    /** A sub-page's title when the name is long ("Program & equipment"): serif 40, wraps. */
    val titleM = OpenType.title.copy(fontSize = 40.sp, lineHeight = 44.sp, letterSpacing = (-1.5).sp)

    /** A chip's label and the quiet text actions (Reset…): Noto Sans 14, 600. */
    val chip = OpenType.body.copy(fontWeight = FontWeight.SemiBold, lineHeight = 18.sp)

    /** An outlined small button's label (What's new, Open): Noto Sans 13, 600. */
    val smallButton = OpenType.body.copy(fontSize = 13.sp, lineHeight = 17.sp, fontWeight = FontWeight.SemiBold)

    /** The lock screen's one line under "Locked": body at 15. */
    val lockCaption = OpenType.body.copy(fontSize = 15.sp, lineHeight = 21.sp)
}

object SettingsColors {
    /** The one erasing action's words (Factory reset), as drawn: a soft red, not the accent. */
    val Danger = Color(0xFFE57373)
}
