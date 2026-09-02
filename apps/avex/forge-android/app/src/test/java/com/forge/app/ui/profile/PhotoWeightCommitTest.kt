package com.forge.app.ui.profile

import com.forge.app.domain.units.WeightUnit
import com.forge.app.domain.units.weightInputValue
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Test

/**
 * M-19: the photo viewer's weight field must not write null over a stored weight because the text
 * was a typo. Blank is the one way to clear; nonblank text that does not parse to a plausible
 * bodyweight keeps what is already committed.
 *
 * M-20: the decision is made against the LAST COMMITTED value, not the value the viewer opened
 * with, so editing A to B and back to A commits A instead of leaving B on disk.
 */
class PhotoWeightCommitTest {

    @Test
    fun textMatchingTheCommittedValueWritesNothing() {
        assertSame(WeightCommit.Keep, weightCommitDecision("170", 170.0, WeightUnit.LB))
        // Whitespace is not an edit.
        assertSame(WeightCommit.Keep, weightCommitDecision("  170  ", 170.0, WeightUnit.LB))
        // An empty field over no stored weight is also untouched, not a clear.
        assertSame(WeightCommit.Keep, weightCommitDecision("", null, WeightUnit.LB))
    }

    @Test
    fun aFieldSeededInDisplayUnitsIsNotAnEditAfterRounding() {
        // The field is seeded through weightInputValue, which rounds to the display step. Comparing
        // in raw lb would read that rounding as a change and silently rewrite the snapshot.
        val seeded = weightInputValue(170.0, WeightUnit.KG)
        assertSame(WeightCommit.Keep, weightCommitDecision(seeded, 170.0, WeightUnit.KG))
    }

    @Test
    fun blankIsTheOnlyClear() {
        assertEquals(WeightCommit.Set(null), weightCommitDecision("", 170.0, WeightUnit.LB))
        assertEquals(WeightCommit.Set(null), weightCommitDecision("   ", 170.0, WeightUnit.LB))
    }

    @Test
    fun invalidNonblankTextKeepsTheStoredWeight() {
        // The audit's reproduction: text the field filter admits but the parser rejects.
        assertSame(WeightCommit.Invalid, weightCommitDecision(".", 170.0, WeightUnit.LB))
        assertSame(WeightCommit.Invalid, weightCommitDecision("1..2", 170.0, WeightUnit.LB))
        assertSame(WeightCommit.Invalid, weightCommitDecision("-", 170.0, WeightUnit.LB))
        // Parseable but not a plausible adult bodyweight — a fat-fingered "8" for "180".
        assertSame(WeightCommit.Invalid, weightCommitDecision("8", 170.0, WeightUnit.LB))
        assertSame(WeightCommit.Invalid, weightCommitDecision("5000", 170.0, WeightUnit.LB))
        // And with nothing stored, invalid text still writes nothing rather than a null.
        assertSame(WeightCommit.Invalid, weightCommitDecision(".", null, WeightUnit.LB))
    }

    @Test
    fun aValidChangeIsWritten() {
        assertEquals(WeightCommit.Set(185.0), weightCommitDecision("185", 170.0, WeightUnit.LB))
        assertEquals(WeightCommit.Set(185.0), weightCommitDecision("185", null, WeightUnit.LB))
    }

    @Test
    fun editingAToBAndBackToACommitsA() {
        // A -> B is a write, and the caller records B as committed. Judged against B, the return to
        // A is a change again; judged against the launch snapshot A it would have looked untouched
        // and left B on disk (M-20).
        val a = 170.0
        val b = weightCommitDecision("185", a, WeightUnit.LB)
        assertEquals(WeightCommit.Set(185.0), b)
        val backToA = weightCommitDecision("170", (b as WeightCommit.Set).lb, WeightUnit.LB)
        assertEquals(WeightCommit.Set(a), backToA)
    }
}
