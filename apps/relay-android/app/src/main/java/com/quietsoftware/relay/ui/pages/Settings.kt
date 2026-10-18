package com.quietsoftware.relay.ui.pages

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.BuildConfig
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.b
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.data.Relay.Live
import com.quietsoftware.relay.service.LinkService
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.SectionLabel
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.kit.Toggle
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.term.MAX_SP
import com.quietsoftware.relay.ui.term.MIN_SP
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

private class PcCategory(val key: String, val label: String, val detail: String)

/** The PC's notification categories (tools_settings.rs), in its order. */
private val PC_CATEGORIES = listOf(
    PcCategory("agent_done", "Agent finished", "An agent reports its work done."),
    PcCategory("agent_blocked", "Agent blocked", "An agent waits on you."),
    PcCategory("guardrail", "Guardrail holds", "An action waits for your Allow."),
    PcCategory("integration", "Integration", "Merges and builds finish or fail."),
    PcCategory("provider", "Providers", "Provider updates and sign-in problems."),
    PcCategory("disk", "Disk", "Worktrees and builds use a lot of space."),
    PcCategory("system", "System", "Crashes on a device and other engine news."),
)

/** The phone's own preferences. The PC's settings stay on the PC. */
@Composable
fun SettingsScreen(nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val p = s.prefs
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    // The first name is typed here, so the field keeps what was typed while the store catches up.
    var name by remember { mutableStateOf<String?>(null) }
    val pcNotify by remember { nav.relay.live("notify.settings.get") }.collectAsStateWithLifecycle(initialValue = Live())
    val categories = (pcNotify.result as? JsonObject)?.o("categories")
    val overrides = remember(pcNotify.result) { mutableStateMapOf<String, Boolean>() }

    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar("Settings", nav::back)
            ScrollBody {
                SectionLabel("Appearance")
                listOf(
                    Triple("matte", "Matte", "Soft console around darker panes"),
                    Triple("dark", "Warm dark", "Easy on long nights"),
                    Triple("oled", "OLED", "True black"),
                ).forEach { (key, label, hint) ->
                    PaletteCard(label, hint, selected = p.palette == key, palette = Palette.of(key)) {
                        nav.shell.update { it.copy(palette = key) }
                    }
                    Gap(8.dp)
                }
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(14.dp)) {
                    T("Your first name", style = Relay.type.body, color = c.ink, weight = FontWeight.Medium)
                    T("Used in the greeting on the start screen.", style = Relay.type.caption, color = c.ink3)
                    Gap(10.dp)
                    Field(
                        name ?: p.name,
                        { v ->
                            name = v
                            nav.shell.update { prefs -> prefs.copy(name = v) }
                        },
                        Modifier.fillMaxWidth(),
                        placeholder = "First name",
                    )
                }

                SectionLabel("Connection")
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                    SettingRow(
                        "Stay connected",
                        "Keeps the link open in the background so agents that need you can say so. Shows an ongoing notification.",
                    ) {
                        Toggle(p.stayConnected, { on ->
                            nav.shell.update { prefs -> prefs.copy(stayConnected = on) }
                            if (on) LinkService.start(context) else LinkService.stop(context)
                        })
                    }
                    Hairline()
                    SettingRow("Notifications", "Tell me when an agent is held, blocked or done while the app is not on screen.") {
                        Toggle(p.notify, { on -> nav.shell.update { prefs -> prefs.copy(notify = on) } })
                    }
                }

                SectionLabel("Terminal")
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                    SettingRow(
                        "Fit the terminal to this phone",
                        "Borrow the PC's terminal at the phone's width while one is open.",
                    ) {
                        Toggle(p.fitTerminal, { on -> nav.shell.update { prefs -> prefs.copy(fitTerminal = on) } })
                    }
                    Hairline()
                    SettingRow("Keep the screen on in a terminal", "The screen stays awake while a terminal is open.") {
                        Toggle(p.keepScreenOn, { on -> nav.shell.update { prefs -> prefs.copy(keepScreenOn = on) } })
                    }
                    Hairline()
                    SettingRow("Type straight to the terminal", "Each key goes as it is pressed, for vim, a shell or a menu, with Ctrl and Alt on the key row. Off, a message box sends a line at a time.") {
                        Toggle(p.terminalDirect, { on -> nav.shell.update { prefs -> prefs.copy(terminalDirect = on) } })
                    }
                    Hairline()
                    SettingRow("Extra keys in a conversation", "Arrows, Tab, Space, Enter and Ctrl-C over an agent's message box.") {
                        Toggle(p.chatKeys, { on -> nav.shell.update { prefs -> prefs.copy(chatKeys = on) } })
                    }
                    Hairline()
                    SettingRow("Text size", "In a terminal, from ${MIN_SP.toInt()} to ${MAX_SP.toInt()} points; a pinch changes it too.") {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            IconKey(
                                "minus",
                                { nav.shell.update { prefs -> prefs.copy(terminalFont = (prefs.terminalFont - 1f).coerceIn(MIN_SP, MAX_SP)) } },
                                enabled = p.terminalFont > MIN_SP,
                            )
                            T(
                                "${p.terminalFont.toInt()} sp",
                                Modifier.width(52.dp),
                                Relay.type.mono.copy(textAlign = TextAlign.Center),
                                c.ink2,
                            )
                            IconKey(
                                "plus",
                                { nav.shell.update { prefs -> prefs.copy(terminalFont = (prefs.terminalFont + 1f).coerceIn(MIN_SP, MAX_SP)) } },
                                enabled = p.terminalFont < MAX_SP,
                            )
                        }
                    }
                }

                SectionLabel("PC")
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                    SettingRow("This PC", "Pairing and the link to your PC", onClick = { nav.pc() }) {
                        Glyph("chevron-right", 16.dp, c.ink3)
                    }
                }

                SectionLabel("Notifications on the PC")
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                    when {
                        pcNotify.result == null -> Row(Modifier.padding(vertical = 12.dp)) {
                            T(pcNotify.error?.message ?: "Loading…", style = Relay.type.caption, color = c.ink3)
                        }
                        else -> PC_CATEGORIES.forEachIndexed { index, category ->
                            if (index > 0) Hairline()
                            val on = overrides[category.key] ?: (categories?.b(category.key) ?: true)
                            SettingRow(category.label, category.detail) {
                                Toggle(
                                    on,
                                    { value ->
                                        scope.launch {
                                            try {
                                                nav.relay.call(
                                                    "notify.settings.set",
                                                    buildJsonObject {
                                                        put("patch", buildJsonObject { put("categories", buildJsonObject { put(category.key, value) }) })
                                                    },
                                                )
                                                overrides[category.key] = value
                                            } catch (e: BusException) {
                                                nav.shell.toast(e.error.message.ifBlank { e.error.code })
                                            }
                                        }
                                    },
                                    enabled = s.online,
                                )
                            }
                        }
                    }
                }
                Gap(6.dp)
                T(
                    "These are the PC's notifications, which this phone also shows. Their sound and volume are set on the PC.",
                    style = Relay.type.caption,
                    color = c.ink3,
                )

                SectionLabel("About")
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                    SettingRow("Relay for Android", "Version ${BuildConfig.VERSION_NAME}")
                    Hairline()
                    Column(Modifier.padding(vertical = 10.dp)) {
                        T("Fonts: Geist, Geist Mono, Sora, Fira (SIL OFL)", style = Relay.type.caption, color = c.ink3)
                    }
                }
            }
        }
    }
}

@Composable
private fun SettingRow(
    title: String,
    hint: String?,
    onClick: (() -> Unit)? = null,
    control: @Composable () -> Unit = {},
) {
    val c = Relay.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = TOUCH)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            T(title, style = Relay.type.body, color = c.ink, weight = FontWeight.Medium)
            hint?.let { T(it, style = Relay.type.caption, color = c.ink3) }
        }
        Spacer(Modifier.width(12.dp))
        control()
    }
}

/** A palette as a card: a swatch of its ground and slab, its name and what it is for. */
@Composable
private fun PaletteCard(label: String, hint: String, selected: Boolean, palette: Palette, onClick: () -> Unit) {
    val c = Relay.colors
    Slab(
        Modifier.fillMaxWidth(),
        padding = PaddingValues(14.dp),
        edge = if (selected) c.ink else c.edge,
        onClick = onClick,
    ) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier
                    .size(34.dp)
                    .clip(Radii.key)
                    .background(palette.wall)
                    .border(1.dp, palette.strong, Radii.key)
                    .padding(7.dp),
            ) {
                Box(Modifier.fillMaxSize().clip(Radii.keycap).background(palette.slab))
            }
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                T(label, style = Relay.type.uiMedium, color = c.ink)
                T(hint, style = Relay.type.caption, color = c.ink3)
            }
            if (selected) Glyph("check", 16.dp, c.ink)
        }
    }
}
