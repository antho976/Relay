package com.tally.app.ui.theme

import androidx.compose.ui.graphics.Color

// ── Pearl, inherited from Avex: a WARM near-black (red channel highest) ─────────────────────────
val PearlBackground = Color(0xFF110F0C)
val PearlSurface = Color(0xFF1A1613)
val PearlSurfaceVar = Color(0xFF221C16)
val PearlOutline = Color(0xFF38302A)
val PearlOnBg = Color(0xFFF2EFEA)      // 16.7:1 on the ground
val PearlMuted = Color(0xFFBFB6AA)     // 9.6:1 full, 4.65:1 at 0.65 (the floor; 0.6 fails)

// ── State colours: true states only, never decoration ─────────────────────────────────────────
/** Over budget. As a mark it clears 3:1; never body text (3.7:1). */
val TallyError = Color(0xFFD9534A)
val TallySuccess = Color(0xFF6FB98A)

/**
 * The category hues: a data series, not decoration. Twelve mid-tones that each clear 3:1 as a
 * mark on the ground and stay apart from one another, so a breakdown bar reads by colour at a
 * glance. Indexed by CategoryEntity.color; order is stored, so append, never reorder.
 */
val CategoryPalette = listOf(
    Color(0xFF7FB27A), // 0 sage
    Color(0xFF4FA9A0), // 1 teal
    Color(0xFF6A9FD8), // 2 sky
    Color(0xFF8C87D9), // 3 iris
    Color(0xFFC27BC0), // 4 orchid
    Color(0xFFD9768E), // 5 rose
    Color(0xFFE08A5F), // 6 coral
    Color(0xFFD9A441), // 7 amber
    Color(0xFFA3A84E), // 8 olive
    Color(0xFFC2A585), // 9 sand
    Color(0xFF8D99A6), // 10 slate
    Color(0xFFB8664F), // 11 brick
)

fun categoryColor(index: Int?): Color = CategoryPalette[((index ?: 10) % CategoryPalette.size + CategoryPalette.size) % CategoryPalette.size]
