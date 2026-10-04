package com.margin.app.ui.categories

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ReceiptLong
import androidx.compose.material.icons.rounded.CalendarToday
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.EditNote
import androidx.compose.material.icons.rounded.Inventory2
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.margin.app.ui.accounts.FieldRow
import com.margin.app.ui.common.Caption
import com.margin.app.ui.common.CategoryBadge
import com.margin.app.ui.common.ChromeButton
import com.margin.app.ui.common.Dates
import com.margin.app.ui.common.GUTTER
import com.margin.app.ui.common.Group
import com.margin.app.ui.common.GroupBlock
import com.margin.app.ui.common.HeroAction
import com.margin.app.ui.common.HeroPanel
import com.margin.app.ui.common.LocalMoney
import com.margin.app.ui.common.PANEL_GAP
import com.margin.app.ui.common.PaceMeter
import com.margin.app.ui.common.PageTitle
import com.margin.app.ui.common.StatChip
import com.margin.app.ui.common.SwitchRow
import com.margin.app.ui.common.TopBar
import com.margin.app.ui.nav.AppNav
import com.margin.app.ui.theme.categoryColor
import com.margin.core.Copy

@Composable
fun CategoryEditRoute(nav: AppNav) {
    val viewModel: CategoryEditViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    LaunchedEffect(viewModel) { viewModel.done.collect { nav.back() } }
    val actions = remember(viewModel, nav) {
        CategoryEditActions(
            back = nav::back,
            setName = viewModel::setName,
            setIcon = viewModel::setIcon,
            setColor = viewModel::setColor,
            setArchived = viewModel::setArchived,
            save = viewModel::save,
            requestDelete = viewModel::requestDelete,
            dismissMove = viewModel::dismissMove,
            delete = viewModel::delete,
        )
    }
    CategoryEditScreen(state, actions)
}

/** Everything the category editor can do, as plain lambdas. */
data class CategoryEditActions(
    val back: () -> Unit = {},
    val setName: (String) -> Unit = {},
    val setIcon: (String) -> Unit = {},
    val setColor: (Int) -> Unit = {},
    val setArchived: (Boolean) -> Unit = {},
    val save: () -> Unit = {},
    val requestDelete: () -> Unit = {},
    val dismissMove: () -> Unit = {},
    /** Deletes, moving the entries to the given category, or leaving them uncategorized on null. */
    val delete: (Long?) -> Unit = {},
)

/**
 * The category editor: the category as it will look, built live under a light in its own hue,
 * then its name, glyph, hue and whether it shows in pickers.
 */
@Composable
fun CategoryEditScreen(state: CategoryEditState, actions: CategoryEditActions) {
    val d = state.draft
    Column(Modifier.fillMaxSize().imePadding().navigationBarsPadding()) {
        TopBar(onBack = actions.back) {
            if (!state.isNew) ChromeButton(Icons.Rounded.DeleteOutline, "Delete category", actions.requestDelete)
        }
        Column(
            Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(start = GUTTER, end = GUTTER, top = 4.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
        ) {
            PageTitle(
                if (state.isNew) "New category" else "Edit category",
                context = listOfNotNull(
                    "For " + kindLabel(state.kind).lowercase(),
                    if (state.isNew) null else Copy.plural(state.entryCount, "entry", "entries"),
                ).joinToString(" · "),
            )
            if (state.loaded) {
                CategoryPreview(state)
                val problem = state.shownNameProblem
                val nameRow: @Composable (Shape) -> Unit = { shape ->
                    FieldRow(
                        d.name,
                        actions.setName,
                        "Category name",
                        Icons.Rounded.EditNote,
                        shape,
                        capitalization = KeyboardCapitalization.Sentences,
                        isError = problem != null,
                    )
                }
                Group(rows = listOf(nameRow), title = "Name", footer = problem, footerIsError = true)
                val hue = categoryColor(d.color)
                val iconBlock: @Composable (Shape) -> Unit = { shape ->
                    GroupBlock(shape) { IconGrid(d.icon, hue, actions.setIcon) }
                }
                Group(rows = listOf(iconBlock), title = "Icon", trailing = iconName(d.icon))
                val hueBlock: @Composable (Shape) -> Unit = { shape ->
                    GroupBlock(shape) { HueSwatches(d.color, actions.setColor) }
                }
                Group(rows = listOf(hueBlock), title = "Colour", trailing = hueName(d.color))
                val archivedRow: @Composable (Shape) -> Unit = { shape ->
                    SwitchRow(
                        "Archived",
                        d.archived,
                        actions.setArchived,
                        shape,
                        subtitle = "Hidden from pickers, kept in history",
                    )
                }
                Group(rows = listOf(archivedRow), title = "Visibility")
                HeroAction("Save category", actions.save, Modifier.fillMaxWidth())
            }
        }
    }
    val moving = state.moving
    if (moving != null) {
        MoveSheet(
            name = state.storedName.ifEmpty { "This category" },
            entries = moving,
            targets = state.targets,
            onMove = actions.delete,
            onDismiss = actions.dismissMove,
        )
    }
}

/**
 * The thing being built: its badge at full size and its name in serif, lit in its own hue, then
 * its share of this period's money for the kind and chips for what it already holds.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun CategoryPreview(state: CategoryEditState, modifier: Modifier = Modifier) {
    val money = LocalMoney.current
    val d = state.draft
    val hue = categoryColor(d.color)
    val periodName = Dates.period(state.period, state.today)
    val what = kindLabel(state.kind).lowercase()
    val percent = Math.round(state.share * 100f)
    val shareText = when {
        state.isNew -> "A new category starts with no entries"
        state.periodTotal == 0L -> "Nothing in it for $periodName yet"
        else -> "$percent% of $periodName's $what, ${money.formatWhole(state.periodTotal)} of ${money.formatWhole(state.kindTotal)}"
    }
    val name = d.name.trim()
    HeroPanel(modifier, tint = hue) {
        Row(
            Modifier.fillMaxWidth().semantics(mergeDescendants = true) {
                contentDescription = "Preview: " + name.ifEmpty { "unnamed" } + ", " + iconName(d.icon) + ", " + hueName(d.color)
                liveRegion = LiveRegionMode.Polite
            },
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            CategoryBadge(d.icon, d.color, size = 64.dp)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(
                    name.ifEmpty { "Name it below" },
                    style = MaterialTheme.typography.headlineMedium,
                    color = if (name.isEmpty()) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onBackground,
                )
                Text(
                    (kindLabel(state.kind) + " category").uppercase(),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Spacer(Modifier.height(18.dp))
        PaceMeter(state.share, null, shareText, height = 8.dp, fill = hue)
        Spacer(Modifier.height(8.dp))
        Caption(shareText)
        Spacer(Modifier.height(14.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(
                Icons.AutoMirrored.Rounded.ReceiptLong,
                if (state.isNew) "No entries yet" else Copy.plural(state.entryCount, "entry", "entries"),
            )
            if (!state.isNew) StatChip(Icons.Rounded.CalendarToday, money.formatWhole(state.periodTotal) + " in " + periodName)
            if (d.archived) StatChip(Icons.Rounded.Inventory2, "Archived")
        }
    }
}
