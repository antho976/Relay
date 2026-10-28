package com.quietsoftware.relay.ui.code

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The phone's design Ask writes what the PC's does (ask.rs `prompt`, its tests) for a whole page. */
class DesignAskTest {
    private fun ask(request: String, title: String = "Settings", pageName: String = "Flows") = designPrompt(title, 12, "page-7", pageName, request)

    @Test fun the_prompt_is_the_pcs_for_a_whole_page() {
        assertEquals(
            "[Relay design] \"Settings\" (design_id 12, page \"Flows\" (page-7)). Nothing selected: about the whole page page-7." +
                " Request: Tidy — read it with design.get {\"design_id\":12,\"page\":\"page-7\"} and change it with design.apply.",
            ask("Tidy"),
        )
    }

    @Test fun names_carry_no_keys_and_the_request_keeps_its_lines() {
        val p = ask("\n\n a \n\n\n\tb\u001b\n\n", title = "Save \"now\"\u001b[A", pageName = "Box\t1")
        assertTrue(p, p.startsWith("[Relay design] \"Save 'now' [A\" (design_id 12, page \"Box 1\" (page-7))."))
        assertTrue(p, p.contains("Request: a\n\nb —"))
        assertEquals(p, 2, p.count { it == '\n' })
    }
}
