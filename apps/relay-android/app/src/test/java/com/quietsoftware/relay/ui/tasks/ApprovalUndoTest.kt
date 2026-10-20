package com.quietsoftware.relay.ui.tasks

import com.quietsoftware.relay.core.wire.s
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Reopen and Undo take back the approval that put a task in Done, and no older one. */
class ApprovalUndoTest {
    private fun history(vararg rows: String) = Json.parseToJsonElement(rows.joinToString(",", "[", "]")).jsonArray

    private val approved = """{"op":"task.approve","kind":"ok","undone_by":null,"undo_op":{"op":"task.unapprove","payload":{"task_id":7,"column":"in_review","position":2,"state":"awaiting_review","sha":"abc1234"}}}"""

    @Test
    fun theNewestApprovalIsTakenBack() {
        val undo = approvalUndoIn(history(approved, """{"op":"task.move","kind":"ok","payload":{"task_id":7,"column":"in_review"}}"""))
        assertEquals("in_review", undo?.s("column"))
        assertEquals("abc1234", undo?.s("sha"))
    }

    @Test
    fun anApprovalReopenedFromThePhoneIsNotTakenBackAgain() {
        // Approved, reopened here (which marks nothing undone), then moved to Done by hand.
        assertNull(approvalUndoIn(history("""{"op":"task.move","kind":"ok","payload":{"task_id":7,"column":"done"}}""", """{"op":"task.unapprove","kind":"ok","payload":{"task_id":7,"column":"in_review"}}""", approved)))
        assertNull(approvalUndoIn(history("""{"op":"task.unapprove","kind":"ok","payload":{"task_id":7,"column":"in_review"}}""", approved)))
    }

    @Test
    fun aMoveToDoneSinceTheApprovalHasNoInverse() {
        assertNull(approvalUndoIn(history("""{"op":"task.move","kind":"ok","payload":{"task_id":7,"column":"done"}}""", approved)))
    }

    @Test
    fun anApprovalUndoneOnThePcOrRefusedIsNotTakenBack() {
        assertNull(approvalUndoIn(history(approved.replace("\"undone_by\":null", "\"undone_by\":91"))))
        // A refused approval changed nothing: the one before it is the one that counts.
        val refused = """{"op":"task.approve","kind":"refused","undone_by":null}"""
        assertEquals("in_review", approvalUndoIn(history(refused, approved))?.s("column"))
    }
}
