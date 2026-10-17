package com.forge.app.ui.settings

import org.junit.Assert.assertEquals
import org.junit.Test

class BackupSubtitleTest {

    @Test fun `on with a copy names its date`() {
        assertEquals("Weekly · last copy Oct 12, 2026", backupSubtitle(enabled = true, savedAt = "Oct 12, 2026", failed = false))
    }

    @Test fun `on with no copy yet says so`() {
        assertEquals("Weekly · no copy yet", backupSubtitle(enabled = true, savedAt = null, failed = false))
    }

    @Test fun `a failure leads whatever else is true`() {
        assertEquals("Last backup failed · back up now", backupSubtitle(enabled = true, savedAt = "Oct 12, 2026", failed = true))
        assertEquals("Last backup failed · back up now", backupSubtitle(enabled = false, savedAt = null, failed = true))
    }

    @Test fun `off keeps the last copy's date`() {
        assertEquals("Weekly copy off · last Oct 12, 2026", backupSubtitle(enabled = false, savedAt = "Oct 12, 2026", failed = false))
        assertEquals("Weekly copy off", backupSubtitle(enabled = false, savedAt = null, failed = false))
    }
}
