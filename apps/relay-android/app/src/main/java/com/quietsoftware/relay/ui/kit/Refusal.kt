package com.quietsoftware.relay.ui.kit

import com.quietsoftware.relay.core.wire.BusError
import com.quietsoftware.relay.core.wire.BusException

/** What an act says with the PC away, for keys that check before they ask. */
const val NEEDS_PC = "Needs the PC, which is out of reach"

/**
 * What a refused or failed act says to the person, the one wording every screen uses: plainly
 * when the PC is away, where to decide a held one, and the engine's own sentence with its hint.
 */
fun refusal(e: BusError): String = when {
    e.code == "link.down" -> NEEDS_PC
    e.kind == "held" -> "Held for approval on the PC. Decide it in the inbox."
    e.message.isNotBlank() -> e.hint?.takeIf { it.isNotBlank() }?.let { "${e.message} ($it)" } ?: e.message
    else -> e.code
}

fun refusal(e: BusException): String = refusal(e.error)
