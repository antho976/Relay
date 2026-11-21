package com.forge.app.ui.common

import androidx.compose.foundation.clickable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.semantics

/**
 * A labeled, role-tagged [clickable] for accessibility — TalkBack reads the [label] and announces
 * the [role]. Centralizes the `clickable(onClickLabel = …, role = Role.Button)` pattern that was
 * repeated at every tappable Text/Row/Box, so the a11y defaults live in one place and a future
 * change (a min touch target, extra semantics) lands once instead of at ~30 call sites.
 */
fun Modifier.clickableLabeled(
    label: String,
    role: Role = Role.Button,
    onClick: () -> Unit
): Modifier = clickable(onClickLabel = label, role = role, onClick = onClick)

/**
 * The switch equivalent of [clickableLabeled] — makes a whole ROW the toggle's tap target, carrying
 * the label, `Role.Switch`, and the on/off state so TalkBack announces the value rather than just
 * "button". Use it wherever a settings row owns a [ForgeSwitch]: the switch itself draws a 40×24dp
 * track, well under §14's 48dp minimum, so it is drawn (`onCheckedChange = null`) and the row is
 * what you actually hit. Keeps §2③'s one-tap-target-per-row rule at the same time.
 */
fun Modifier.toggleableLabeled(
    label: String,
    checked: Boolean,
    onToggle: () -> Unit
): Modifier = toggleable(
    value = checked,
    role = Role.Switch,
    onValueChange = { onToggle() }
).semantics { onClick(label = label, action = null) }
