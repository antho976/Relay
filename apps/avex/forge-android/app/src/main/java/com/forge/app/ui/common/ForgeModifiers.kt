package com.forge.app.ui.common

import androidx.compose.foundation.clickable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role

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
