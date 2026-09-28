package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.ForgeChromeButton
import com.forge.app.ui.common.ForgeChromeIconButton
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.memberFill

/*
 * Profile's private pieces of the grouped-surface kit (`ui/common/ForgeGroups.kt`): a stepper or
 * top-bar glyph capsule that can go passive, and the filled chip a many-option filter wears.
 */

/** A glyph chrome capsule. One that cannot act renders passive and says so, never a dead button. */
@Composable
internal fun ProfileIconCapsule(
    icon: ImageVector,
    label: String,
    enabled: Boolean = true,
    tint: Color = MaterialTheme.colorScheme.onBackground,
    onClick: () -> Unit
) {
    if (enabled) {
        ForgeChromeIconButton(icon, label, onClick, tint)
    } else {
        Box(
            Modifier
                .minimumInteractiveComponentSize()
                .heightIn(min = 44.dp)
                .widthIn(min = 44.dp)
                .clip(RoundedCornerShape(50))
                .background(MaterialTheme.colorScheme.surfaceContainerHigh.copy(alpha = 0.35f))
                .semantics { contentDescription = label; disabled() },
            contentAlignment = Alignment.Center
        ) {
            Icon(
                icon,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f),
                modifier = Modifier.size(20.dp)
            )
        }
    }
}

/** A small filled capsule carrying a text label: a one-shot or navigation action. */
@Composable
internal fun ProfileTextCapsule(
    text: String,
    onClick: () -> Unit,
    label: String = text,
    color: Color = MaterialTheme.colorScheme.onBackground,
    modifier: Modifier = Modifier
) {
    ForgeChromeButton(onClick = onClick, label = label, modifier = modifier) {
        Text(
            text,
            style = MaterialTheme.typography.bodyMedium,
            color = color,
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 6.dp)
        )
    }
}

/** A filled chip for a filter set too long (or multi-select) for segments. */
@Composable
internal fun ProfileFilledChip(
    text: String,
    selected: Boolean,
    onClick: () -> Unit,
    leading: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
    role: Role = Role.Checkbox
) {
    val shape = RoundedCornerShape(50)
    Box(
        Modifier
            .minimumInteractiveComponentSize()
            .memberFill(shape, selected)
            .bounceClick(onClick = onClick)
            .semantics { this.selected = selected; this.role = role }
            .padding(horizontal = 14.dp, vertical = 8.dp),
        contentAlignment = Alignment.Center
    ) {
        androidx.compose.foundation.layout.Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(6.dp)
        ) {
            leading?.invoke()
            Text(
                text,
                style = MaterialTheme.typography.bodyMedium,
                color = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant
            )
            trailing?.invoke()
        }
    }
}
