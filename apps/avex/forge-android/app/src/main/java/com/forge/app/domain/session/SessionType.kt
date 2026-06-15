package com.forge.app.domain.session

/**
 * The session-type marker persisted on [com.forge.app.data.db.entities.Session.sessionType] (#109).
 * Single source of truth for the canonical stored keys and their Overview status-pill labels, so the
 * write side and the pill can't drift on bare string literals — an unknown key now fails to resolve
 * via [fromKey] instead of silently showing no chip (or the wrong one).
 *
 * Note: this is the SESSION-type namespace ("normal"/"deload"/"test"/…). It is unrelated to the
 * coach-decision `type` strings ("deload"/"swap"/"rep_shift"/…), which are a separate vocabulary.
 */
enum class SessionType(val key: String, val pillLabel: String?) {
    NORMAL("normal", null),
    DELOAD("deload", "DELOAD"),
    TEST("test", "TEST"),
    TECHNIQUE("technique", "TECHNIQUE"),
    FIRST_BACK("first_back", "FIRST BACK");

    companion object {
        /** The type for a stored key, or null if it isn't a recognized marker. */
        fun fromKey(key: String): SessionType? = entries.firstOrNull { it.key == key }
    }
}
