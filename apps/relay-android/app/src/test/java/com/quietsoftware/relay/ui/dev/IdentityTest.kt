package com.quietsoftware.relay.ui.dev

import com.quietsoftware.relay.core.model.Project
import com.quietsoftware.relay.core.model.Session
import com.quietsoftware.relay.core.model.Task
import org.junit.Assert.assertEquals
import org.junit.Test

/** How an agent is told apart from the others: its initials, its colour, its title and its line. */
class IdentityTest {
    private val otter = Session(name = "brisk-otter", projectId = 1)

    @Test fun initials_and_colour_come_from_the_name_alone() {
        assertEquals("BO", agentInitials("brisk-otter"))
        assertEquals("WO", agentInitials("worker"))
        assertEquals(agentHue("brisk-otter"), agentHue("brisk-otter"))
    }

    @Test fun the_title_is_its_intent_then_its_task_then_its_name() {
        val task = Task(id = "12", title = "Fix the board")
        assertEquals("Porting", agentTitle(otter.copy(intent = "Porting"), task))
        assertEquals("Fix the board", agentTitle(otter.copy(intent = " "), task))
        assertEquals("brisk-otter", agentTitle(otter))
    }

    @Test fun the_line_names_what_the_title_does_not() {
        val project = Project(id = "1", name = "RelayV4")
        assertEquals("RelayV4 · reviewer · #12", agentWhere(otter.copy(role = "reviewer"), "brisk-otter", project, Task(id = "12")))
        assertEquals("brisk-otter", agentWhere(otter, "Porting", null, null))
    }
}
