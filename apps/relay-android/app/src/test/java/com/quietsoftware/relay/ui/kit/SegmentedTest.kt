package com.quietsoftware.relay.ui.kit

import org.junit.Assert.assertEquals
import org.junit.Test

/** The view switch keeps every label whole: it wraps rather than cutting "Medium" to "Medi…". */
class SegmentedTest {
    @Test fun `tabs that fit stay on one line`() {
        assertEquals(listOf(listOf(0, 1, 2)), segmentLines(listOf(40, 50, 60), gap = 2, room = 154))
    }

    @Test fun `tabs that do not fit wrap in order`() {
        // Backlog, Ready, Active, In review at a large font on a narrow phone.
        assertEquals(listOf(listOf(0, 1), listOf(2, 3)), segmentLines(listOf(90, 70, 80, 110), gap = 2, room = 200))
        // One wider than the room has a line of its own.
        assertEquals(listOf(listOf(0), listOf(1), listOf(2)), segmentLines(listOf(50, 300, 50), gap = 2, room = 200))
        assertEquals(emptyList<List<Int>>(), segmentLines(emptyList(), gap = 2, room = 200))
    }

    @Test fun `a spread line shares the width equally while every label fits its share`() {
        assertEquals(listOf(66, 66, 66), segmentWidths(listOf(40, 50, 60), gap = 2, span = 202))
        // Rounding goes to the last.
        assertEquals(listOf(66, 66, 67), segmentWidths(listOf(40, 50, 60), gap = 2, span = 203))
    }

    @Test fun `a label wider than its share keeps its width and the rest is shared`() {
        // 196 inside: 20, 30 and 120 leave 26, so 8 each and the last 2 for the rounding.
        assertEquals(listOf(28, 38, 130), segmentWidths(listOf(20, 30, 120), gap = 2, span = 200))
        // Not spread: each at its own width.
        assertEquals(listOf(20, 30, 120), segmentWidths(listOf(20, 30, 120), gap = 2, span = 174))
    }
}
