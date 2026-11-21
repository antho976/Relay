package com.forge.app.ui.goals

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.OpenTopBarIcon
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.theme.openOnAccentLarge

/*
 * Small form pieces the redesigned Goals, Recap, Notifications and Check-in pages share. Internal to
 * the app module rather than in ui/common, which holds only the kit DESIGN §8 lists.
 */

private val Corner = RoundedCornerShape(4.dp)

/** The page's top row: back on the left, at most one glyph on the right. Never the page's name. */
@Composable
internal fun OpenBackBar(onBack: () -> Unit, modifier: Modifier = Modifier, action: (@Composable () -> Unit)? = null) {
    Row(
        modifier
            .fillMaxWidth()
            .windowInsetsPadding(TopAppBarDefaults.windowInsets)
            .padding(horizontal = 24.dp)
            .height(48.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        OpenTopBarIcon(OpenIcons.ArrowLeft, "Back", onBack)
        Spacer(Modifier.weight(1f))
        action?.invoke()
    }
}

/**
 * A wide action, 52dp, its label centred: filled red with white (the design's Set goal and Save) or
 * outlined (Later). Disabled draws outlined and does nothing.
 */
@Composable
internal fun OpenWideButton(
    text: String,
    filled: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    val on = filled && enabled
    val ink = if (on) openOnAccentLarge() else MaterialTheme.colorScheme.onBackground
    Box(
        modifier
            .heightIn(min = 52.dp)
            .bounceClick(enabled = enabled, role = Role.Button, onClick = onClick)
            .clip(Corner)
            .then(
                if (on) Modifier.background(MaterialTheme.colorScheme.primary)
                else Modifier.border(1.dp, OpenColors.Rule, Corner)
            )
            .padding(horizontal = 16.dp, vertical = 12.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text,
            style = if (filled) OpenType.button else OpenType.rowTitle,
            color = if (enabled) ink else MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center
        )
    }
}

/**
 * The redesign's switch, drawn only (the row it sits in owns the tap, so TalkBack meets one
 * control): on is a red pill with a white knob at the end, off an outline with a grey knob.
 */
@Composable
internal fun OpenSwitchMark(checked: Boolean, modifier: Modifier = Modifier) {
    Box(
        modifier
            .size(width = 40.dp, height = 24.dp)
            .clip(CircleShape)
            .then(
                if (checked) Modifier.background(MaterialTheme.colorScheme.primary)
                else Modifier.border(1.5.dp, OpenColors.Rule, CircleShape)
            )
            .padding(horizontal = if (checked) 3.dp else 5.dp),
        contentAlignment = if (checked) Alignment.CenterEnd else Alignment.CenterStart
    ) {
        Box(
            Modifier
                .size(if (checked) 18.dp else 14.dp)
                .background(if (checked) openOnAccentLarge() else OpenColors.Glyph, CircleShape)
        )
    }
}

/**
 * An outlined 40dp input: the typed value, a muted placeholder while empty, and a mono unit at its
 * end ("lb"). [label] is what TalkBack calls the field.
 */
@Composable
internal fun OpenField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    placeholder: String,
    modifier: Modifier = Modifier,
    suffix: String? = null,
    isError: Boolean = false,
    width: Dp? = null,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
) {
    val ink = MaterialTheme.colorScheme.onBackground
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        singleLine = true,
        textStyle = OpenType.body.copy(color = ink),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
        modifier = modifier
            .then(if (width != null) Modifier.size(width = width, height = 40.dp) else Modifier.fillMaxWidth().height(40.dp))
            .semantics { contentDescription = label },
        decorationBox = { inner ->
            Row(
                Modifier
                    .fillMaxWidth()
                    .height(40.dp)
                    .border(1.dp, if (isError) MaterialTheme.colorScheme.error else OpenColors.Rule, Corner)
                    .padding(horizontal = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
                    if (value.isEmpty()) Text(placeholder, style = OpenType.body, color = OpenColors.Glyph)
                    inner()
                }
                if (suffix != null) Text(suffix.uppercase(), style = OpenType.monoXS, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    )
}
