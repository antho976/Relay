package com.quietsoftware.relay.ui.code

import com.quietsoftware.relay.core.design.DesignIcons
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.GraphicsMode

/** Every icon the PC draws parses on the phone into a path on its 24-unit grid. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class DesignIconsTest {
    @Test
    fun `every icon parses into its grid`() {
        val names = DesignIcons.names
        assertTrue("${names.size}", names.size >= 70)
        for (name in names) {
            val path = iconPath(DesignIcons.path(name)!!)
            assertNotNull(name, path)
            val b = path!!.getBounds()
            assertTrue("$name: $b", b.width > 0f || b.height > 0f)
            assertTrue("$name: $b", b.left >= -0.01f && b.top >= -0.01f && b.right <= 24.01f && b.bottom <= 24.01f)
        }
    }

    @Test
    fun `a name the set lacks has no path`() {
        assertEquals(null, DesignIcons.path("no-such-icon"))
    }
}
