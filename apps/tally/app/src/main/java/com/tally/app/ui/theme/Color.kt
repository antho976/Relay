package com.tally.app.ui.theme

import androidx.compose.ui.graphics.Color

/**
 * The category hues: a data series, not decoration (Relay's `Palette.HUES`, money_pages.rs). Twelve
 * mid-tones that each clear 3:1 as a mark on Relay's ground and stay apart from one another, so a
 * breakdown bar reads by colour at a glance. Indexed by CategoryEntity.color; the order is stored
 * and synced, so append, never reorder.
 */
val CategoryPalette = listOf(
    Color(0xFF13A36E), // 0 sage
    Color(0xFF0EA5A0), // 1 teal
    Color(0xFF2F7CF6), // 2 sky
    Color(0xFF6C4BF4), // 3 iris
    Color(0xFFB25BC4), // 4 orchid
    Color(0xFFF0508C), // 5 rose
    Color(0xFFFF6A3D), // 6 coral
    Color(0xFFE89A12), // 7 amber
    Color(0xFF8C9A2E), // 8 olive
    Color(0xFFB98A5E), // 9 sand
    Color(0xFF6B7A8C), // 10 slate
    Color(0xFFC2452D), // 11 brick
)

fun categoryColor(index: Int?): Color = CategoryPalette[((index ?: 10) % CategoryPalette.size + CategoryPalette.size) % CategoryPalette.size]
