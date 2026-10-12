package com.tally.app.ui.invest

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.kit.DetailPage
import com.tally.app.ui.kit.Dot
import com.tally.app.ui.kit.Empty
import com.tally.app.ui.kit.Eyebrow
import com.tally.app.ui.kit.GlyphBadge
import com.tally.app.ui.kit.IconKey
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.PaceMeter
import com.tally.app.ui.kit.Panel
import com.tally.app.ui.kit.Pill
import com.tally.app.ui.kit.Slab
import com.tally.app.ui.kit.SpacePage
import com.tally.app.ui.kit.StackBar
import com.tally.app.ui.kit.Stat
import com.tally.app.ui.kit.StatGrid
import com.tally.app.ui.kit.T
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.plan.PadKeys
import com.tally.app.ui.plan.padText
import com.tally.app.ui.theme.Tally
import com.tally.app.ui.theme.categoryColor

@Composable
fun InvestRoute(nav: AppNav) {
    val vm: InvestViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { vm.onResume() }
    InvestScreen(
        s,
        import = { nav.import("investments") },
        account = { nav.transactions(accountId = it) },
        addAccount = { nav.accountEdit() },
        room = { nav.roomEdit(it) },
    )
}

/** The investment accounts: what they are worth, what that cost, where it sits, and this year's room. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun InvestScreen(
    s: InvestState,
    import: () -> Unit,
    account: (Long) -> Unit,
    addAccount: () -> Unit,
    room: (com.tally.core.Registration) -> Unit,
) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    var lensKey by rememberSaveable { mutableStateOf<String?>(null) }
    val v = investView(s.portfolio, s.accounts, lensKey)
    SpacePage(
        "Investments",
        context = if (s.loaded) investContext(v.accounts.size, v.newest, s.today) { Dates.short(it, s.today) } else null,
        actions = { IconKey("download", import, label = "Import a Wealthsimple file") },
    ) {
        if (!s.loaded) return@SpacePage
        s.problem?.let { T(it, style = Tally.type.caption, color = c.heldText) }
        if (v.accounts.isEmpty()) {
            Slab {
                Empty("trend", "No investment accounts", "Add a TFSA, an RRSP or a brokerage account, then import its Wealthsimple files or record what it is worth.") {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Key("Add an account", addAccount, glyph = "plus")
                        Key("Import", import, glyph = "download", kind = KeyKind.Quiet)
                    }
                }
            }
            return@SpacePage
        }
        if (v.lenses.isNotEmpty()) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Pill("All", selected = v.lens == null, onClick = { lensKey = null })
                v.lenses.forEach { l -> Pill(l.label, selected = v.lens?.key == l.key, onClick = { lensKey = l.key }) }
            }
        }
        Slab(Modifier.fillMaxWidth(), padding = PaddingValues(16.dp), spacing = 6.dp) {
            Eyebrow(v.lens?.label ?: "Worth")
            T(fmt.formatWhole(v.worth), style = Tally.type.hero, color = c.ink, maxLines = 1)
            if (v.hasHoldings) {
                T(gainText(v.gain, v.gainBps, fmt) + " on " + fmt.formatWhole(v.cost) + " put in", style = Tally.type.caption, color = if (v.gain >= 0) c.liveText else c.heldText)
            }
            v.newest?.let { if (isStale(it, s.today)) T(staleLine(it, s.today), style = Tally.type.caption, color = c.waiting) }
        }
        StatGrid(
            listOf(
                { m -> Stat("Cash", fmt.formatWhole(v.cash), m, glyph = "coins") },
                { m -> Stat("Income, 12 months", v.income12m?.let { fmt.formatWhole(it) } ?: "–", m, glyph = "money-in", sub = "dividends and interest") },
            ),
        )
        if (v.allocation.isNotEmpty()) {
            Panel("By account kind", glyph = "pie") {
                StackBar(v.allocation.map { categoryColor(it.hue) to it.value }, Modifier.padding(4.dp))
                v.allocation.forEach { line -> ShareRow(line.label, line.hue, fmt.formatWhole(line.value), sharePercent(line.shareBps)) }
            }
        }
        if (v.kinds.isNotEmpty()) {
            Panel("By kind of holding", glyph = "bars") {
                StackBar(v.kinds.map { categoryColor(it.hue) to it.value }, Modifier.padding(4.dp))
                v.kinds.forEach { line -> ShareRow(line.label, line.hue, fmt.formatWhole(line.value), sharePercent(line.shareBps)) }
            }
        }
        Panel("Accounts", glyph = "bank") {
            v.accounts.forEach { a ->
                ListRow(onClick = { account(a.id) }, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
                    GlyphBadge("trend", categoryColor(registrationHue(a.registration)), categoryColor(registrationHue(a.registration)).copy(alpha = .16f))
                    Column(Modifier.weight(1f)) {
                        T(a.name, style = Tally.type.ui, color = c.ink, maxLines = 1)
                        T(accountMeta(a) { Dates.short(it, s.today) }, style = Tally.type.caption, color = c.ink3, maxLines = 1)
                    }
                    Column(horizontalAlignment = Alignment.End) {
                        T(fmt.formatWhole(a.worth), style = Tally.type.amount, color = c.ink)
                        returnText(a) { Dates.short(it, s.today) }?.let { T(it, style = Tally.type.mono, color = c.ink3) }
                    }
                }
            }
        }
        if (v.holdings.isNotEmpty()) {
            Panel("Holdings", glyph = "list", subtitle = "Largest first") {
                v.holdings.take(20).forEach { h ->
                    Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        GlyphBadge("coins", categoryColor(kindHue(h.kind)), categoryColor(kindHue(h.kind)).copy(alpha = .16f))
                        Column(Modifier.weight(1f)) {
                            T(h.symbol + if (h.name.isNotBlank()) " · " + h.name else "", style = Tally.type.ui, color = c.ink, maxLines = 1)
                            T(holdingLine(h, h.weightBps), style = Tally.type.caption, color = c.ink3, maxLines = 1)
                        }
                        Column(horizontalAlignment = Alignment.End) {
                            T(fmt.formatWhole(h.value), style = Tally.type.amount, color = c.ink)
                            T(gainText(h.gain, h.gainBps, fmt), style = Tally.type.mono, color = if (h.gain >= 0) c.liveText else c.heldText)
                        }
                    }
                }
            }
        }
        if (v.room.isNotEmpty()) {
            Panel("Room this year", glyph = "flag", subtitle = "From CRA My Account; Tally subtracts what goes in") {
                v.room.forEach { line ->
                    ListRow(onClick = { room(line.registration) }, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(5.dp)) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                T(line.registration.label + " " + line.year, Modifier.weight(1f), Tally.type.ui, c.ink)
                                T(roomReading(line, fmt), style = Tally.type.amount, color = if (line.over > 0) c.heldText else c.ink2)
                            }
                            val room = line.room
                            if (room != null && room > 0) PaceMeter(line.contributed.toFloat() / room, roomPaceFraction(line.registration, line.year, s.today))
                            T(roomText(line, s.today, fmt) { Dates.short(it, s.today) }, style = Tally.type.caption, color = if (line.over > 0) c.heldText else c.ink3, maxLines = 2)
                        }
                    }
                }
            }
        }
        v.issues.forEach { T(it, style = Tally.type.caption, color = c.waiting) }
    }
}

@Composable
private fun ShareRow(label: String, hue: Int, value: String, share: String) {
    val c = Tally.colors
    Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 5.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Dot(categoryColor(hue), 8.dp)
        T(label, Modifier.weight(1f), Tally.type.caption, c.ink2, maxLines = 1)
        T(value, style = Tally.type.mono, color = c.ink2)
        T(share, style = Tally.type.mono, color = c.ink3)
    }
}

@Composable
fun RoomEditRoute(nav: AppNav) {
    val vm: RoomEditViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    LaunchedEffect(vm) { vm.done.collect { nav.back() } }
    val fmt = LocalMoney.current
    DetailPage(
        s.registration.label + " room " + s.year,
        nav::back,
        subtitle = "Deadline " + Dates.long(s.deadline),
        actions = { if (s.existing != null) IconKey("trash", vm::remove, label = "Remove the figure") },
        footer = {
            PadKeys(s.input.fractionDigits, fmt.decimalSeparator, vm::press)
            Key("Save room", vm::save, Modifier.fillMaxWidth(), kind = KeyKind.Primary, enabled = s.input.minor > 0, glyph = "check")
        },
    ) {
        if (!s.loaded) return@DetailPage
        Eyebrow("Room for " + s.year)
        T(padText(s.input, fmt), style = Tally.type.hero.copy(fontSize = 40.sp), color = if (s.input.isEmpty) Tally.colors.ink3 else Tally.colors.ink, maxLines = 1)
        T(roomSource(s.registration), style = Tally.type.caption, color = Tally.colors.ink3)
        T(fmt.formatWhole(s.contributed) + " went in this year so far", style = Tally.type.caption, color = Tally.colors.ink2)
        s.limit?.let { Pill("This year's limit " + fmt.formatWhole(it), glyph = "sparkle", onClick = { vm.fill(it) }) }
    }
}
