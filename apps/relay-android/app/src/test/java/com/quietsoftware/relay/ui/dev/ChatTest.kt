package com.quietsoftware.relay.ui.dev

import com.quietsoftware.relay.core.model.ThreadMessage
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
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
            tool("3", "Edit", "src/main.rs"), answer("3", error = true),
            tool("4", "Bash", "cargo build"), answer("4"),
            msg("w", "assistant", """{"blocks":[{"type":"text","text":"Fixed."}]}"""),
            tool("5", "Bash", "cargo test"),
        )
        val pieces = chatPieces(log, emptyList(), working = true, today = today)
        assertEquals(listOf(ChatPiece.User::class, ChatPiece.Steps::class, ChatPiece.Reply::class, ChatPiece.Tool::class), pieces.map { it::class })
        val steps = (pieces[1] as ChatPiece.Steps).calls
        assertEquals(listOf(Step.Done, Step.Done, Step.Failed, Step.Done), steps.map { it.step })
        assertEquals("Ran 2 commands, read 1 file, edited 1 file", stepsLine(steps))
        assertEquals(Step.Running, (pieces[3] as ChatPiece.Tool).call.step)
        // The same log once the agent stopped: the unanswered call was cut off.
        assertEquals(Step.Cut, (chatPieces(log, emptyList(), working = false, today = today)[3] as ChatPiece.Tool).call.step)
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
}
