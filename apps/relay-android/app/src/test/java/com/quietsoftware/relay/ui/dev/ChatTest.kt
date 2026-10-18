package com.quietsoftware.relay.ui.dev

import com.quietsoftware.relay.core.model.ThreadMessage
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate

/** A terminal agent drawn as a conversation: its log in pieces, and its question read off its screen. */
class ChatTest {
    private fun msg(id: String, role: String, body: String) = ThreadMessage(id, role = role, body = Json.parseToJsonElement(body), createdAt = "2026-10-13T10:00:00Z")

    private fun tool(id: String, name: String, summary: String) =
        msg("a$id", "assistant", """{"blocks":[{"type":"tool_use","id":"t$id","name":"$name","summary":"$summary"}]}""")

    private fun answer(id: String, error: Boolean = false) = msg("r$id", "tool", """{"tool_use_id":"t$id","is_error":$error,"text":"out $id"}""")

    private val today = LocalDate.parse("2026-10-13")

    @Test
    fun stepsInARowFoldUnderOneLineAndWordsStandAlone() {
        val log = listOf(
            msg("u", "user", """{"text":"fix the build"}"""),
            tool("1", "Bash", "cargo build"), answer("1"),
            tool("2", "Read", "src/main.rs"), answer("2"),
            tool("3", "Grep", "fn main"), answer("3", error = true),
            tool("4", "Bash", "cargo build"), answer("4"),
            msg("w", "assistant", """{"blocks":[{"type":"text","text":"Fixed."}]}"""),
            tool("5", "Bash", "cargo test"),
        )
        val pieces = chatPieces(log, emptyList(), working = true, today = today)
        assertEquals(listOf(ChatPiece.User::class, ChatPiece.Steps::class, ChatPiece.Reply::class, ChatPiece.Tool::class), pieces.map { it::class })
        val steps = (pieces[1] as ChatPiece.Steps).calls
        assertEquals(listOf(Step.Done, Step.Done, Step.Failed, Step.Done), steps.map { it.step })
        assertEquals("Ran 2 commands, read 1 file, ran 1 search", stepsLine(steps))
        assertEquals(Step.Running, (pieces[3] as ChatPiece.Tool).call.step)
        // The same log once the agent stopped: the unanswered call was cut off.
        assertEquals(Step.Cut, (chatPieces(log, emptyList(), working = false, today = today)[3] as ChatPiece.Tool).call.step)
    }

    @Test
    fun reasoningAndEditsStandAloneWithWhatTheyChanged() {
        val log = listOf(
            msg("u", "user", """{"text":"rename it"}"""),
            msg("a", "assistant", """{"blocks":[{"type":"thinking","text":"The name is used twice."},{"type":"tool_use","id":"t1","name":"Read","summary":"a.rs"}]}"""),
            answer("1"),
            msg("b", "assistant", """{"blocks":[{"type":"tool_use","id":"t2","name":"Edit","summary":"a.rs","detail":"-old\n+new\n+more"}]}"""),
            answer("2"),
            tool("3", "Bash", "cargo build"), answer("3"),
        )
        val pieces = chatPieces(log, emptyList(), working = false, today = today)
        assertEquals(listOf(ChatPiece.User::class, ChatPiece.Thought::class, ChatPiece.Tool::class, ChatPiece.Tool::class, ChatPiece.Tool::class), pieces.map { it::class })
        assertEquals("The name is used twice.", (pieces[1] as ChatPiece.Thought).text)
        val edit = (pieces[3] as ChatPiece.Tool).call
        assertEquals("-old\n+new\n+more", edit.detail)
        assertEquals(2 to 1, diffCounts(edit.detail!!.lines()))
        assertEquals(null, (pieces[4] as ChatPiece.Tool).call.detail)
    }

    @Test
    fun aQuestionStillOpenWaitsOnThePersonEvenWhileHeld() {
        val log = listOf(
            msg("u", "user", """{"text":"plan it"}"""),
            msg("a", "assistant", """{"blocks":[{"type":"tool_use","id":"t1","name":"ExitPlanMode","summary":"","detail":"1. Do it"}]}"""),
        )
        // Held at the prompt, the call waits; with the turn over, it was cut off.
        val held = chatPieces(log, emptyList(), working = true, today = today)
        assertEquals(Step.Running, (held.last() as ChatPiece.Tool).call.step)
        assertTrue(waitsOnAsk(held))
        assertFalse(waitsOnAsk(chatPieces(log, emptyList(), working = false, today = today)))
    }

    @Test
    fun aMenuOnScreenAsksAndANumberedListDoesNot() {
        val menu = """
            Select model
            ❯ 1. Default (recommended)
              2. Opus
              3. Haiku
        """.trimIndent()
        assertTrue(screenAsks(menu))
        assertTrue(screenAsks("Codex wants to run a command\n› 1. Yes, proceed\n  2. No"))
        assertTrue(screenAsks("Overwrite the file? (y/n)"))
        assertFalse(screenAsks("Done. Next:\n1. Add a test\n2. Ship it\n\n> \n  ⏵⏵ accept edits on"))
        assertFalse(screenAsks(""))
        // A message of the person's, echoed with the prompt mark, is not a menu.
        assertFalse(screenAsks("❯ 1. fix the build\n\n> "))
    }

    @Test
    fun whatThePersonSentShowsUntilTheLogHasIt() {
        val sent = msg("tmp:1", "user", """{"text":"and the tests"}""")
        val pieces = chatPieces(emptyList(), listOf(sent), working = false, today = today)
        assertEquals(ChatPiece.User("m:tmp:1", "and the tests", sending = true), pieces.single())
    }

    @Test
    fun aPermissionPromptBecomesItsQuestionAndNumberedAnswers() {
        val screen = """
            ╭──────────────────────────────────────────────╮
            │ Bash command                                 │
            │                                              │
            │   cargo test -p relay-core                   │
            │                                              │
            │ Do you want to proceed?                      │
            │ ❯ 1. Yes                                     │
            │   2. Yes, and don't ask again for cargo test │
            │   3. No, and tell Claude what to do (esc)    │
            ╰──────────────────────────────────────────────╯
        """.trimIndent()
        val (question, options) = askOf(screen)
        assertEquals(listOf("Bash command", "cargo test -p relay-core", "Do you want to proceed?"), question)
        assertEquals(listOf("1" to "Yes", "2" to "Yes, and don't ask again for cargo test", "3" to "No, and tell Claude what to do (esc)"), options)
    }

    @Test
    fun aScreenWithNoChoicesIsShownAsItsLastLines() {
        val (question, options) = askOf("working…\n\nOverwrite the file? (y/n)\n\n")
        assertTrue(options.isEmpty())
        assertEquals(listOf("working…", "Overwrite the file? (y/n)"), question)
    }

    @Test
    fun theModeIsReadOffClaudeCodesFooterOnly() {
        val prompt = "╭────────╮\n│ >      │\n╰────────╯\n"
        assertEquals("Accept edits", modeOf("⏺ Done.\n\n$prompt  ⏵⏵ accept edits on (shift+tab to cycle)\n"))
        assertEquals("Plan", modeOf("$prompt  ⏸ plan mode on (shift+tab to cycle)"))
        assertEquals("Default", modeOf("$prompt  ? for shortcuts"))
        // Words in the conversation above the prompt are not the footer.
        assertEquals(null, modeOf("I turned plan mode on for you\n\nmore\nlines\nhere\n$prompt"))
    }

    @Test
    fun aFinishedTurnEndsWithACopyOfItsWordsAndOneStillRunningDoesNot() {
        val log = listOf(
            msg("u1", "user", """{"text":"why"}"""),
            msg("a1", "assistant", """{"blocks":[{"type":"text","text":"Because."},{"type":"tool_use","id":"t1","name":"Read","summary":"a.rs"}]}"""),
            answer("1"),
            msg("a2", "assistant", """{"blocks":[{"type":"text","text":"And so."}]}"""),
            msg("u2", "user", """{"text":"and then"}"""),
            msg("a3", "assistant", """{"blocks":[{"type":"text","text":"Then this."}]}"""),
        )
        val done = chatPieces(log, emptyList(), working = false, today = today)
        val copies = done.filterIsInstance<ChatPiece.Copy>()
        assertEquals(listOf("Because.\n\nAnd so.", "Then this."), copies.map { it.text })
        // The copy closes its turn, before the person's next message.
        assertTrue(done[done.indexOf(copies[0]) + 1] is ChatPiece.User)
        val running = chatPieces(log, emptyList(), working = true, today = today)
        assertEquals(1, running.count { it is ChatPiece.Copy })
        assertTrue(running.last() is ChatPiece.Reply)
    }

    @Test
    fun aTurnsTimeReadsAsClaudeCodeCountsIt() {
        assertEquals("42s", took(42_000))
        assertEquals("3m 12s", took(192_000))
        assertEquals("1h 4m", took(3_840_000))
        assertEquals(null, took(-5))
        assertEquals(null, took(13 * 3_600_000L))
    }

    @Test
    fun aSlashAndOneWordAsksForACommandNamesFirst() {
        assertEquals("", commandAsked("/"))
        assertEquals("com", commandAsked("/com"))
        assertEquals(null, commandAsked("/compact keep the plan"))
        assertEquals(null, commandAsked("fix /tmp"))
        val all = listOf("compact" to "Summarise the conversation", "context" to "What fills it", "clear" to "Start over with nothing in context")
        assertEquals(listOf("compact", "context", "clear"), matchCommands(all, "c").map { it.first })
        // A name holding the word beats a description that does.
        assertEquals(listOf("context", "clear"), matchCommands(all, "context").map { it.first })
    }

    @Test
    fun picturesGoAsPathsAndComeBackAsPills() {
        val path = "/w/brisk-otter/.relay/attachments/phone-0123456789abcdef.jpg"
        val sent = withAttachments("  what is wrong here?  ", listOf(path))
        assertEquals("what is wrong here?\n\n$path", sent)
        assertEquals("what is wrong here?" to listOf("phone-0123456789abcdef.jpg"), splitAttached(sent))
        // A picture alone, and a message that only mentions the folder.
        assertEquals(path, withAttachments("", listOf(path)))
        assertEquals("" to listOf("phone-0123456789abcdef.jpg"), splitAttached(path))
        assertEquals("see .relay/attachments/ for them" to emptyList<String>(), splitAttached("see .relay/attachments/ for them"))
    }

    @Test
    fun theWorkingRowNamesAStepOnlyWhenItIsFoldedOutOfSight() {
        val alone = chatPieces(listOf(msg("u", "user", """{"text":"go"}"""), tool("1", "Bash", "cargo test")), emptyList(), working = true, today = today)
        assertEquals("Working…", doingNow(alone))
        val folded = chatPieces(
            listOf(msg("u", "user", """{"text":"go"}"""), tool("1", "Read", "a.rs"), answer("1"), tool("2", "Read", "b.rs"), answer("2"), tool("3", "Bash", "cargo test")),
            emptyList(), working = true, today = today,
        )
        assertEquals("Running cargo test…", doingNow(folded))
    }

    @Test
    fun whatWasSaidJoinsWhatIsTyped() {
        assertEquals("run the tests", joinSaid("", "run the tests"))
        assertEquals("first fix it then run the tests", joinSaid("first fix it  ", "then run the tests"))
    }
}
