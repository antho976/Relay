package com.quietsoftware.relay.data

import android.content.Context
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.floatPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

private val Context.settingsStore by preferencesDataStore(name = "settings")

/** The phone's own preferences. The PC's settings stay on the PC. */
data class Prefs(
    /** Keep the link open in the background (an ongoing notification), so agents that need you can say so. */
    val stayConnected: Boolean = true,
    /** Notify when an agent is held, blocked or done while the app is not on screen. */
    val notify: Boolean = true,
    val keepScreenOn: Boolean = true,
    /** Borrow the PC's terminal at the phone's own width while one is open. */
    val fitTerminal: Boolean = true,
    /** The background family (`Palette.BASES`); the PC's `matte`, `dark` and `oled` still read. */
    val palette: String = "grey",
    /** The accent (`Accent`): primary keys, the send key, switches, selections. */
    val accent: String = "clay",
    val terminalFont: Float = 11f,
    /** The space the app opened last: `dev` or `threads`. */
    val space: String = "dev",
    /** The person's first name, for the start screen's greeting. */
    val name: String = "",
    val lastProject: Long = 0,
    /** The agent conversation's row of extra keys (arrows, Tab, Space, ^C) shown over its message box. */
    val chatKeys: Boolean = false,
    /** The terminal takes the keyboard key by key (vim, a shell, a TUI's menus) instead of a message box. */
    val terminalDirect: Boolean = false,
)

class Settings(private val context: Context) {
    val prefs: Flow<Prefs> = context.settingsStore.data.map(::read)

    private fun read(p: Preferences) = Prefs(
        stayConnected = p[STAY] ?: true,
        notify = p[NOTIFY] ?: true,
        keepScreenOn = p[SCREEN] ?: true,
        fitTerminal = p[FIT] ?: true,
        palette = p[PALETTE] ?: "grey",
        accent = p[ACCENT] ?: "clay",
        terminalFont = p[FONT] ?: 11f,
        space = p[SPACE] ?: "dev",
        name = p[NAME] ?: "",
        lastProject = p[PROJECT]?.toLongOrNull() ?: 0,
        chatKeys = p[CHAT_KEYS] ?: false,
        terminalDirect = p[DIRECT] ?: false,
    )

    suspend fun update(change: (Prefs) -> Prefs) {
        context.settingsStore.edit { p ->
            val next = change(read(p))
            p[STAY] = next.stayConnected
            p[NOTIFY] = next.notify
            p[SCREEN] = next.keepScreenOn
            p[FIT] = next.fitTerminal
            p[PALETTE] = next.palette
            p[ACCENT] = next.accent
            p[FONT] = next.terminalFont
            p[SPACE] = next.space
            p[NAME] = next.name
            p[PROJECT] = next.lastProject.toString()
            p[CHAT_KEYS] = next.chatKeys
            p[DIRECT] = next.terminalDirect
        }
    }

    private companion object {
        val STAY = booleanPreferencesKey("stay_connected")
        val NOTIFY = booleanPreferencesKey("notify")
        val SCREEN = booleanPreferencesKey("keep_screen_on")
        val FIT = booleanPreferencesKey("fit_terminal")
        val PALETTE = stringPreferencesKey("palette")
        val ACCENT = stringPreferencesKey("accent")
        val FONT = floatPreferencesKey("terminal_font")
        val SPACE = stringPreferencesKey("space")
        val NAME = stringPreferencesKey("name")
        val PROJECT = stringPreferencesKey("last_project")
        val CHAT_KEYS = booleanPreferencesKey("chat_keys")
        val DIRECT = booleanPreferencesKey("terminal_direct")
    }
}
