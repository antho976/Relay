package com.forge.app.ui.common

import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.lazy.LazyItemScope
import androidx.compose.foundation.lazy.LazyListItemInfo
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.zIndex

/**
 * Minimal hand-rolled drag-to-reorder for a [androidx.compose.foundation.lazy.LazyColumn] — no
 * third-party dependency. Long-press any row to pick it up, then drag; rows it passes shift live via
 * [onMove], which the caller applies to its backing list. Built for short lists (a handful of days,
 * a handful of exercises) so it skips edge auto-scroll. Rows MUST have stable `key`s for the
 * placement animation + index tracking to work.
 *
 * When the LazyColumn has fixed leading items above the draggable rows (headers, hints, pickers),
 * pass their count as [firstDraggableIndex] so the state can translate between full LazyColumn item
 * indices (what the layout reports) and the 0-based backing-list indices the caller expects in
 * [onMove] / [DraggableItem]. Without this the dragged row never highlights and the reorder swaps
 * the wrong elements (or no-ops when the shifted index falls out of range).
 *
 * Usage:
 * ```
 * val listState = rememberLazyListState()
 * val drag = rememberDragDropState(listState, firstDraggableIndex = HEADER_COUNT) { from, to ->
 *     items = items.move(from, to)
 * }
 * LazyColumn(state = listState, modifier = Modifier.dragContainer(drag)) {
 *     item { Header() }                            // HEADER_COUNT leading items
 *     itemsIndexed(items, key = { _, it -> it.id }) { i, item ->
 *         DraggableItem(drag, i) { dragging -> Row(...) }
 *     }
 * }
 * ```
 */
@Composable
fun rememberDragDropState(
    lazyListState: LazyListState,
    firstDraggableIndex: Int = 0,
    onMove: (Int, Int) -> Unit
): DragDropState =
    remember(lazyListState, firstDraggableIndex) { DragDropState(lazyListState, firstDraggableIndex, onMove) }

class DragDropState internal constructor(
    private val state: LazyListState,
    /** Count of fixed leading items (headers) before the draggable rows in the LazyColumn. */
    private val firstDraggableIndex: Int,
    private val onMove: (Int, Int) -> Unit
) {
    /** Full LazyColumn index of the row being dragged (includes [firstDraggableIndex]); null = none. */
    var draggingItemIndex by mutableStateOf<Int?>(null)
        private set

    private var draggingItemDraggedDelta by mutableFloatStateOf(0f)
    private var draggingItemInitialOffset by mutableIntStateOf(0)

    /** True when the row at backing-list [dataIndex] is the one currently being dragged. */
    internal fun isDragging(dataIndex: Int): Boolean = draggingItemIndex == dataIndex + firstDraggableIndex

    /** Live vertical translation to apply to the dragged row so it follows the finger. */
    internal val draggingItemOffset: Float
        get() = draggingItemLayoutInfo?.let { item ->
            draggingItemInitialOffset + draggingItemDraggedDelta - item.offset
        } ?: 0f

    private val draggingItemLayoutInfo: LazyListItemInfo?
        get() = state.layoutInfo.visibleItemsInfo.firstOrNull { it.index == draggingItemIndex }

    internal fun onDragStart(offset: Offset) {
        state.layoutInfo.visibleItemsInfo
            // Only the draggable rows can be picked up — a long-press on a header is ignored.
            .firstOrNull { it.index >= firstDraggableIndex && offset.y.toInt() in it.offset..(it.offset + it.size) }
            ?.also {
                draggingItemIndex = it.index
                draggingItemInitialOffset = it.offset
            }
    }

    internal fun onDragInterrupted() {
        draggingItemIndex = null
        draggingItemDraggedDelta = 0f
        draggingItemInitialOffset = 0
    }

    internal fun onDrag(offset: Offset) {
        draggingItemDraggedDelta += offset.y
        val dragging = draggingItemLayoutInfo ?: return
        val startOffset = dragging.offset + draggingItemOffset
        val middleOffset = startOffset + dragging.size / 2f
        val target = state.layoutInfo.visibleItemsInfo.firstOrNull { item ->
            // Never target a header slot — only swap among the draggable rows.
            item.index >= firstDraggableIndex && item.index != dragging.index &&
                middleOffset.toInt() in item.offset..(item.offset + item.size)
        }
        if (target != null) {
            // Translate full LazyColumn indices back to backing-list indices for the caller.
            onMove(dragging.index - firstDraggableIndex, target.index - firstDraggableIndex)
            draggingItemIndex = target.index
        }
    }
}

/** Long-press-and-drag gesture for the LazyColumn that hosts [DraggableItem]s. */
fun Modifier.dragContainer(dragDropState: DragDropState): Modifier = pointerInput(dragDropState) {
    detectDragGesturesAfterLongPress(
        onDragStart = { offset -> dragDropState.onDragStart(offset) },
        onDrag = { change, dragAmount -> change.consume(); dragDropState.onDrag(dragAmount) },
        onDragEnd = { dragDropState.onDragInterrupted() },
        onDragCancel = { dragDropState.onDragInterrupted() }
    )
}

/** Wrap each row in this so the dragged one floats and the rest animate into place. [index] is the
 *  0-based backing-list index (the `itemsIndexed` index), not the full LazyColumn index. */
@Composable
fun LazyItemScope.DraggableItem(
    dragDropState: DragDropState,
    index: Int,
    modifier: Modifier = Modifier,
    content: @Composable (isDragging: Boolean) -> Unit
) {
    val dragging = dragDropState.isDragging(index)
    val itemModifier =
        if (dragging) Modifier.zIndex(1f).graphicsLayer { translationY = dragDropState.draggingItemOffset }
        else Modifier.animateItem()
    Box(modifier.then(itemModifier)) { content(dragging) }
}
