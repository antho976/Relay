package com.quietsoftware.relay.ui.pages

import android.content.Context
import android.content.Intent
import android.provider.Settings
import androidx.compose.foundation.background
import com.quietsoftware.relay.ui.kit.refusal
import com.quietsoftware.relay.ui.threads.replyStyle
import com.quietsoftware.relay.ui.theme.Accent
import com.quietsoftware.relay.ui.kit.Eyebrow
import androidx.compose.ui.unit.sp
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.graphics.Color
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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
import androidx.core.app.NotificationManagerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
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
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.SectionLabel
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.kit.Toggle
import com.quietsoftware.relay.ui.kit.switchRow
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

private class PcCategory(val key: String, val label: String, val detail: String)

/** The PC's notification categories, in its words, groups and order (tools_settings.rs): the agents', then the system's. */
private val PC_GROUPS = listOf(
    listOf(
        PcCategory("agent_blocked", "Agent needs you", "Blocked on a question or waiting for approval."),
        PcCategory("agent_done", "Agent finished", "Once per terminal, and again after you type into it."),
        PcCategory("guardrail", "Guardrail needs you", "An action held for approval, or an exception request. Refusals are filed quietly."),
        PcCategory("aide", "Aide", "Your own agent, when it has something you should hear now. The rest waits for its brief."),
    ),
    listOf(
        PcCategory("provider", "Providers", "Claude or Codex limits, outages and sign-in issues."),
        PcCategory("integration", "Integrations", "Git, GitHub and plugin events."),
        PcCategory("disk", "Disk", "Low space and worktree cleanup."),
        PcCategory("system", "Everything else", "Crashed app runs and other engine events."),
    ),
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
    // Android's own switch for Relay's notifications, read again each time the page comes back.
    var allowed by remember { mutableStateOf(NotificationManagerCompat.from(context).areNotificationsEnabled()) }
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { allowed = NotificationManagerCompat.from(context).areNotificationsEnabled() }

    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar("Settings", nav::back)
            ScrollBody {
                SectionLabel("Appearance")
                Appearance(p.palette, p.accent, onBase = { key -> nav.shell.update { it.copy(palette = key) } }, onAccent = { key -> nav.shell.update { it.copy(accent = key) } })
                Gap(12.dp)
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
                    SwitchRow("Stay connected", STAY_CONNECTED, p.stayConnected) { on -> nav.stayConnected(context, on) }
                    Hairline()
                    SwitchRow("Notifications", "Tell me when an agent is held, blocked or done while the app is not on screen.", p.notify) { on ->
                        nav.shell.update { prefs -> prefs.copy(notify = on) }
                    }
                    // On here shows nothing while Android has them off: say so, with the way to its switch.
                    if (p.notify && !allowed) {
                        Row(Modifier.fillMaxWidth().padding(bottom = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                            T("Android has notifications off for Relay, so none show.", Modifier.weight(1f), Relay.type.caption, c.heldText)
                            Spacer(Modifier.width(12.dp))
                            Key("Android settings", {
                                runCatching {
                                    context.startActivity(Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName))
                                }
                            }, glyph = "external", compact = true)
                        }
                    }
                }

                SectionLabel("Conversations")
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                    SwitchRow("Keep the screen on", "The screen stays awake while an agent's conversation is open.", p.keepScreenOn) { on ->
                        nav.shell.update { prefs -> prefs.copy(keepScreenOn = on) }
                    }
                }

                SectionLabel("PC")
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                    SettingRow("Your PC", "Pairing and the link to your PC", onClick = { nav.pc() }) {
                        Glyph("chevron-right", 16.dp, c.ink3)
                    }
                }

                SectionLabel("Notifications on the PC")
                if (pcNotify.result == null) {
                    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                        Row(Modifier.padding(vertical = 12.dp)) {
                            T(pcNotify.error?.message ?: "Loading…", style = Relay.type.caption, color = c.ink3)
                        }
                    }
                } else {
                    PC_GROUPS.forEachIndexed { group, list ->
                        // The agents' notices show on the PC whatever their switch says (tools_settings.rs "Always shown").
                        if (group == 0) {
                            T("Agents' notices always show on the PC; these switch their sound.", style = Relay.type.caption, color = c.ink3)
                            Gap(6.dp)
                        } else {
                            Gap(10.dp)
                        }
                        Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                            list.forEachIndexed { index, category ->
                                if (index > 0) Hairline()
                                val on = overrides[category.key] ?: (categories?.b(category.key) ?: true)
                                SwitchRow(category.label, category.detail, on, enabled = s.online) { value ->
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
                                            nav.shell.toast(refusal(e))
                                        }
                                    }
                                }
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
    tap: Modifier = Modifier,
    control: @Composable () -> Unit = {},
) {
    val c = Relay.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = TOUCH)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else tap)
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

/** A setting that is one switch: a tap anywhere on the row turns it, and it is read as one switch. */
@Composable
private fun SwitchRow(title: String, hint: String?, checked: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit) =
    SettingRow(title, hint, tap = Modifier.switchRow(checked, enabled, onChange)) { Toggle(checked, null, enabled = enabled) }

/**
 * The theme's two choices, Appearance in design 14: a preview of a short exchange in the picked
 * colours, a row of accents, and the background families as tiles, two to a row.
 */
@Composable
internal fun Appearance(base: String, accent: String, onBase: (String) -> Unit, onAccent: (String) -> Unit) {
    val c = Relay.colors
    val picked = Palette.of(base, accent)
    Column(
        Modifier.fillMaxWidth().clip(Radii.card).background(picked.wall).border(1.dp, picked.edge, Radii.card).padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.CenterEnd) {
            T(
                "Is the board test fixed?",
                Modifier.clip(RoundedCornerShape(16.dp, 16.dp, 4.dp, 16.dp)).background(picked.wash).padding(horizontal = 12.dp, vertical = 8.dp),
                Relay.type.ui, picked.ink,
            )
        }
        T("Yes, it passes now. Want me to open a pull request?", style = replyStyle().copy(fontSize = 15.sp, lineHeight = 22.sp), color = picked.ink)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            T("Open it", Modifier.clip(Radii.key).background(picked.accent).padding(horizontal = 14.dp, vertical = 8.dp), Relay.type.uiMedium, picked.onAccent, weight = FontWeight.SemiBold)
            T("Not yet", Modifier.clip(Radii.key).background(picked.slab).padding(horizontal = 14.dp, vertical = 8.dp), Relay.type.uiMedium, picked.ink, weight = FontWeight.SemiBold)
        }
    }
    Gap(14.dp)
    Eyebrow("Accent")
    Gap(8.dp)
    Row(
        Modifier.fillMaxWidth().clip(Radii.card).background(c.slab).border(1.dp, c.edge, Radii.card).padding(horizontal = 8.dp, vertical = 12.dp).selectableGroup(),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        for (a in Accent.entries) {
            val on = a.key == Accent.of(accent).key
            Column(
                Modifier.width(44.dp).selectable(on, role = Role.RadioButton) { onAccent(a.key) }.padding(vertical = 2.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Box(
                    Modifier.size(36.dp).clip(CircleShape).border(2.dp, if (on) c.ink else Color.Transparent, CircleShape).padding(4.dp).clip(CircleShape).background(a.color),
                    contentAlignment = Alignment.Center,
                ) { if (on) Glyph("check", 14.dp, picked.onAccent) }
                T(a.label, style = Relay.type.caption.copy(fontSize = 11.sp), color = if (on) c.ink else c.ink3, weight = if (on) FontWeight.SemiBold else null)
            }
        }
    }
    Gap(14.dp)
    Eyebrow("Background")
    Gap(8.dp)
    Column(Modifier.selectableGroup(), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        for (pair in Palette.BASES.chunked(2)) {
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                for (b in pair) {
                    val on = b.key == Palette.base(base).key
                    Column(
                        Modifier.weight(1f).clip(Radii.card).background(b.ground)
                            .border(if (on) 2.dp else 1.dp, if (on) picked.accent else c.edge, Radii.card)
                            .selectable(on, role = Role.RadioButton) { onBase(b.key) }.padding(12.dp),
                        verticalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            Box(Modifier.size(width = 56.dp, height = 22.dp).clip(Radii.icon).background(b.card))
                            Box(Modifier.size(width = 36.dp, height = 22.dp).clip(Radii.icon).background(b.card2))
                        }
                        T(b.label, style = Relay.type.uiMedium, color = c.ink, weight = FontWeight.SemiBold)
                    }
                }
            }
        }
    }
}

/** What "Stay connected" means, where Settings and the PC's page offer it. */
internal const val STAY_CONNECTED = "Keeps the link open in the background so agents that need you can say so. Shows an ongoing notification."

/** "Stay connected" switched: kept in the settings, and the link's service started or stopped with it. */
internal fun Nav.stayConnected(context: Context, on: Boolean) {
    shell.update { it.copy(stayConnected = on) }
    if (on) LinkService.start(context) else LinkService.stop(context)
}
