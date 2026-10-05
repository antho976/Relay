package com.tally.app.ui.common

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The two ends of a line share it only when the start's needed width, the gap and the end fit.
 * What the start needs is the caller's rule: its longest word for a row's sentence, its whole
 * line for a panel header's label.
 */
class EndsRowTest {

    // "IN AND OUT" at 200%: its longest word is 60px, the whole label 200px; "DAY 14 OF 31" is 150px.
    private val longestWord = 60
    private val wholeLabel = 200
    private val gap = 24
    private val reading = 150
    private val room = 300

    @Test fun aRowKeepsItsReadingBesideWhileNoWordWouldBreak() {
        assertTrue(endsShareLine(longestWord, gap, reading, room))
    }

    @Test fun aPanelLabelThatWouldWrapSendsItsReadingUnder() {
        // The longest-word rule would keep them side by side and stack the label word by word.
        assertFalse(endsShareLine(wholeLabel, gap, reading, room))
    }

    @Test fun aPanelLabelThatFitsWholeKeepsItsReadingBeside() {
        assertTrue(endsShareLine(wholeLabel, gap, reading, room = wholeLabel + gap + reading))
        assertFalse(endsShareLine(wholeLabel, gap, reading, room = wholeLabel + gap + reading - 1))
    }
}
