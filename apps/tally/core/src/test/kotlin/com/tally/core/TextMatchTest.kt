package com.tally.core

import org.junit.Assert.assertEquals
import org.junit.Test

class TextMatchTest {

    @Test fun `every letter becomes the set of its case forms`() {
        assertEquals("*[mM][eE][tT][rR][oO]*", TextMatch.containsPattern("metro"))
        assertEquals("*[éÉ][pP][iI]*", TextMatch.containsPattern("épi"))
        assertEquals("*[Éé][cC][oO][lL][eE]*", TextMatch.containsPattern("École"))
    }

    @Test fun `digits, spaces and marks stay as they are`() {
        assertEquals("*55 [sS][tT]*", TextMatch.containsPattern("55 st"))
        assertEquals("*%*", TextMatch.containsPattern("%"))
    }

    @Test fun `GLOB syntax in the query is matched literally`() {
        assertEquals("*[*]*", TextMatch.containsPattern("*"))
        assertEquals("*[?]*", TextMatch.containsPattern("?"))
        assertEquals("*[[]*", TextMatch.containsPattern("["))
        assertEquals("*]*", TextMatch.containsPattern("]"))
    }

    @Test fun `a blank query is the empty pattern the queries skip`() {
        assertEquals("", TextMatch.containsPattern("   "))
        assertEquals("", TextMatch.prefixPattern(""))
    }

    @Test fun `a prefix pattern anchors at the start and trims`() {
        assertEquals("[Mm][eE]*", TextMatch.prefixPattern(" Me "))
    }
}
