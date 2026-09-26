package com.forge.app.ui.common.window

import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.material3.AlertDialogDefaults
import androidx.compose.material3.BottomSheetDefaults
import androidx.compose.material3.DatePickerColors
import androidx.compose.material3.DatePickerDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.SheetState
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.PopupPositionProvider
import androidx.compose.ui.window.PopupProperties
import com.forge.app.security.LocalAppLockActive
import androidx.compose.material3.AlertDialog as MaterialAlertDialog
import androidx.compose.material3.DatePickerDialog as MaterialDatePickerDialog
import androidx.compose.material3.DropdownMenu as MaterialDropdownMenu
import androidx.compose.material3.ModalBottomSheet as MaterialModalBottomSheet
import androidx.compose.ui.window.Dialog as ComposeDialog
import androidx.compose.ui.window.Popup as ComposePopup

/*
 * Drop-in replacements for every window-owning composable the app uses, which draw nothing while
 * the app lock is up (RELEASE_AUDIT R1, audit 2026-09-26 P1 #1).
 *
 * The lock is an opaque overlay drawn over a still-composed nav host, so the back stack and every
 * screen's state survive an unlock. That works for anything drawn in the activity's own window. A
 * dialog, bottom sheet, dropdown or popup is a SEPARATE Android window stacked above the activity,
 * so no overlay inside the activity can cover it: a "Discard & continue" left open before the phone
 * went to sleep stayed tappable over the lock, and an open photo viewer stayed on screen above it.
 *
 * So these read [LocalAppLockActive] and simply leave their window out while it is true. The open
 * flag that asked for the window lives with the caller and is untouched, so the same dialog is back,
 * in the same place, the moment the user unlocks. Only the parameters the app actually passes are
 * forwarded; everything else keeps Material's own default. A caller that needs another parameter
 * adds it here, which is the point: `LockAwareWindowDoctrineTest` fails the build if a file imports
 * the unguarded Material or Compose version instead.
 */

@Composable
fun AlertDialog(
    onDismissRequest: () -> Unit,
    confirmButton: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    dismissButton: @Composable (() -> Unit)? = null,
    icon: @Composable (() -> Unit)? = null,
    title: @Composable (() -> Unit)? = null,
    text: @Composable (() -> Unit)? = null,
    containerColor: Color = AlertDialogDefaults.containerColor,
    properties: DialogProperties = DialogProperties(),
) {
    if (LocalAppLockActive.current) return
    MaterialAlertDialog(
        onDismissRequest = onDismissRequest,
        confirmButton = confirmButton,
        modifier = modifier,
        dismissButton = dismissButton,
        icon = icon,
        title = title,
        text = text,
        containerColor = containerColor,
        properties = properties,
    )
}

@Composable
fun Dialog(
    onDismissRequest: () -> Unit,
    properties: DialogProperties = DialogProperties(),
    content: @Composable () -> Unit,
) {
    if (LocalAppLockActive.current) return
    ComposeDialog(onDismissRequest = onDismissRequest, properties = properties, content = content)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ModalBottomSheet(
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
    // Remembered here, outside the lock check, so a sheet hidden by the lock keeps its state.
    sheetState: SheetState = rememberModalBottomSheetState(),
    containerColor: Color = BottomSheetDefaults.ContainerColor,
    content: @Composable ColumnScope.() -> Unit,
) {
    if (LocalAppLockActive.current) return
    MaterialModalBottomSheet(
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        sheetState = sheetState,
        containerColor = containerColor,
        content = content,
    )
}

@Composable
fun DropdownMenu(
    expanded: Boolean,
    onDismissRequest: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    MaterialDropdownMenu(
        expanded = expanded && !LocalAppLockActive.current,
        onDismissRequest = onDismissRequest,
        modifier = modifier,
        content = content,
    )
}

@Composable
fun Popup(
    popupPositionProvider: PopupPositionProvider,
    onDismissRequest: (() -> Unit)? = null,
    properties: PopupProperties = PopupProperties(),
    content: @Composable () -> Unit,
) {
    if (LocalAppLockActive.current) return
    ComposePopup(
        popupPositionProvider = popupPositionProvider,
        onDismissRequest = onDismissRequest,
        properties = properties,
        content = content,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DatePickerDialog(
    onDismissRequest: () -> Unit,
    confirmButton: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    dismissButton: @Composable (() -> Unit)? = null,
    colors: DatePickerColors = DatePickerDefaults.colors(),
    content: @Composable ColumnScope.() -> Unit,
) {
    if (LocalAppLockActive.current) return
    MaterialDatePickerDialog(
        onDismissRequest = onDismissRequest,
        confirmButton = confirmButton,
        modifier = modifier,
        dismissButton = dismissButton,
        colors = colors,
        content = content,
    )
}
