package com.tally.app.ui.categories

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.CategoryIcons
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.kit.CategoryBadge
import com.tally.app.ui.kit.CategoryGlyph
import com.tally.app.ui.kit.DetailPage
import com.tally.app.ui.kit.Field
import com.tally.app.ui.kit.IconKey
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.Labelled
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.Meter
import com.tally.app.ui.kit.Panel
import com.tally.app.ui.kit.PickRow
import com.tally.app.ui.kit.Radii
import com.tally.app.ui.kit.Segmented
import com.tally.app.ui.kit.Sheet
import com.tally.app.ui.kit.Slab
import com.tally.app.ui.kit.StackBar
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.ToggleRow
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.CategoryPalette
import com.tally.app.ui.theme.Tally
import com.tally.app.ui.theme.categoryColor
import com.tally.core.CategoryKind
import com.tally.core.Copy

// ── The list ─────────────────────────────────────────────────────────────────

@Composable
fun CategoriesRoute(nav: AppNav) {
    val vm: CategoriesViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    CategoriesScreen(s, back = nav::back, open = { nav.transactions(categoryId = it) }, edit = { id, kind -> nav.categoryEdit(id, kind) })
}

/** Both kinds of category with this month's money in each, and the ones put away. */
@Composable
fun CategoriesScreen(s: CategoriesState, back: () -> Unit, open: (Long) -> Unit, edit: (Long, CategoryKind) -> Unit) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    var kind by rememberSaveable { mutableStateOf(CategoryKind.EXPENSE) }
    val summary = if (kind == CategoryKind.EXPENSE) s.spending else s.income
    DetailPage("Categories", back, subtitle = Dates.period(s.period, s.today), actions = { IconKey("plus", { edit(0, kind) }, label = "New category") }) {
        Segmented(listOf(CategoryKind.EXPENSE to "Spending", CategoryKind.INCOME to "Income"), kind, { kind = it }, Modifier.fillMaxWidth(), fill = true)
        if (!s.loaded) return@DetailPage
        if (summary.segments.isNotEmpty()) {
            Slab(spacing = 8.dp) {
                T(fmt.formatWhole(summary.periodTotal) + " this month across " + Copy.plural(summary.usedThisPeriod, "category", "categories"), style = Tally.type.ui, color = c.ink2)
                StackBar(summary.segments.map { categoryColor(it.color) to it.total })
                summary.mostUsed?.let { T("Most used: " + it.name, style = Tally.type.caption, color = c.ink3) }
            }
        }
        Panel(kindLabel(kind), glyph = "tag", subtitle = Copy.plural(summary.active.size, "category", "categories")) {
            summary.active.forEach { line -> CategoryLineRow(line, onClick = { open(line.id) }, onLongClick = { edit(line.id, line.kind) }) }
        }
        if (summary.archived.isNotEmpty()) {
            Panel("Archived", glyph = "save") {
                summary.archived.forEach { line -> CategoryLineRow(line, onClick = { edit(line.id, line.kind) }, onLongClick = null) }
            }
        }
        T("Tap a category for its entries; hold it to edit.", style = Tally.type.caption, color = c.ink3)
    }
}

@Composable
private fun CategoryLineRow(line: CategoryLine, onClick: () -> Unit, onLongClick: (() -> Unit)?) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    ListRow(onClick = onClick, onLongClick = onLongClick, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
        CategoryBadge(line.icon, line.color)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                T(line.name, Modifier.weight(1f), Tally.type.ui, c.ink, maxLines = 1)
                if (line.periodTotal > 0) T(fmt.formatWhole(line.periodTotal), style = Tally.type.amount, color = c.ink2)
            }
            if (line.periodTotal > 0) Meter(line.percent / 100f, color = categoryColor(line.color), height = 3.dp)
            T(Copy.plural(line.entries, "entry", "entries") + if (line.percent > 0) " · ${line.percent}% of this month" else "", style = Tally.type.caption, color = c.ink3, maxLines = 1)
        }
    }
}

// ── The editor ───────────────────────────────────────────────────────────────

@Composable
fun CategoryEditRoute(nav: AppNav) {
    val vm: CategoryEditViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    LaunchedEffect(vm) { vm.done.collect { nav.back() } }
    CategoryEditScreen(
        s,
        back = nav::back,
        name = vm::setName,
        icon = vm::setIcon,
        color = vm::setColor,
        archived = vm::setArchived,
        save = vm::save,
        requestDelete = vm::requestDelete,
        dismissMove = vm::dismissMove,
        delete = vm::delete,
    )
}

/** A category: its name, its icon from the set the ledger syncs, its hue, archived or not. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun CategoryEditScreen(
    s: CategoryEditState,
    back: () -> Unit,
    name: (String) -> Unit,
    icon: (String) -> Unit,
    color: (Int) -> Unit,
    archived: (Boolean) -> Unit,
    save: () -> Unit,
    requestDelete: () -> Unit,
    dismissMove: () -> Unit,
    delete: (Long?) -> Unit,
) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val d = s.draft
    DetailPage(
        if (s.isNew) "New " + kindLabel(s.kind).lowercase() + " category" else s.storedName,
        back,
        subtitle = if (s.isNew) null else Copy.plural(s.entryCount, "entry", "entries") + " · " + fmt.formatWhole(s.periodTotal) + " this month",
        actions = { if (!s.isNew) IconKey("trash", requestDelete, label = "Delete the category") },
        footer = {
            s.shownNameProblem?.let { T(it, style = Tally.type.caption, color = c.heldText) }
            Key(if (s.isNew) "Add category" else "Save category", save, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "check")
        },
    ) {
        if (!s.loaded) return@DetailPage
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            CategoryBadge(d.icon, d.color, size = 48.dp)
            Box(Modifier.weight(1f)) {
                Field(d.name, name, Modifier.fillMaxWidth(), placeholder = "Groceries, Rent, Salary", error = s.shownNameProblem != null, keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words))
            }
        }
        Labelled("Colour") {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                CategoryPalette.forEachIndexed { i, hue ->
                    Box(
                        Modifier.size(32.dp).clip(CircleShape).background(hue)
                            .border(2.dp, if (d.color == i) c.ink else Color.Transparent, CircleShape)
                            .clickable(role = Role.RadioButton) { color(i) }
                            .semantics { contentDescription = hueName(i) },
                    )
                }
            }
        }
        CategoryIcons.groups.forEach { (group, keys) ->
            Labelled(group) {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    keys.forEach { key ->
                        val on = d.icon == key
                        Box(
                            Modifier.size(40.dp).clip(Radii.badge).background(if (on) categoryColor(d.color).copy(alpha = .22f) else c.slab)
                                .border(1.dp, if (on) categoryColor(d.color) else c.edge, Radii.badge)
                                .clickable(role = Role.RadioButton) { icon(key) }
                                .semantics { contentDescription = iconName(key) },
                            contentAlignment = Alignment.Center,
                        ) {
                            CategoryGlyph(key, if (on) d.color else 10, 20.dp)
                        }
                    }
                }
            }
        }
        if (!s.isNew) {
            Slab(padding = PaddingValues(horizontal = 10.dp, vertical = 4.dp)) {
                ToggleRow("Archived", d.archived, archived, glyph = "save", subtitle = "Kept with its entries, out of the grid")
            }
        }
    }
    s.moving?.let { use ->
        Sheet(onDismiss = dismissMove, title = moveHeading(use)) {
            T(moveNote(s.storedName, use.budget?.let { fmt.format(it) }), style = Tally.type.caption, color = c.ink3)
            s.targets.forEach { t ->
                PickRow(t.name, false, onClick = { delete(t.id) }, sub = Copy.plural(t.entries, "entry", "entries"), leading = { CategoryBadge(t.icon, t.color, size = 28.dp) })
            }
            PickRow(leaveTitle(use), false, onClick = { delete(null) }, sub = leaveSubtitle(use))
        }
    }
}
