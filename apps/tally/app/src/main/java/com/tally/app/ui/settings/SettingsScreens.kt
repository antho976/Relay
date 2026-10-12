package com.tally.app.ui.settings

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
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
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.BuildConfig
import com.tally.app.data.repo.DataResult
import com.tally.app.data.sync.Connect
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.kit.ConfirmDialog
import com.tally.app.ui.kit.DetailPage
import com.tally.app.ui.kit.Dot
import com.tally.app.ui.kit.Eyebrow
import com.tally.app.ui.kit.Field
import com.tally.app.ui.kit.Glyph
import com.tally.app.ui.kit.Hairline
import com.tally.app.ui.kit.IconKey
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.Labelled
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.Panel
import com.tally.app.ui.kit.PickRow
import com.tally.app.ui.kit.Pill
import com.tally.app.ui.kit.Radii
import com.tally.app.ui.kit.SettingRow
import com.tally.app.ui.kit.Slab
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.TallyMark
import com.tally.app.ui.kit.ToggleRow
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.shell.Tone
import com.tally.app.ui.shell.syncLine
import com.tally.app.ui.shell.toneColor
import com.tally.app.ui.theme.Palette
import com.tally.app.ui.theme.Tally
import com.tally.core.BudgetPeriod
import com.tally.core.Copy

// ── Settings ─────────────────────────────────────────────────────────────────

@Composable
fun SettingsRoute(nav: AppNav) {
    val vm: SettingsViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    val data: DataViewModel = hiltViewModel()
    val d by data.state.collectAsStateWithLifecycle()
    LaunchedEffect(data) { data.erased.collect { nav.restart() } }
    var erasing by rememberSaveable { mutableStateOf(false) }
    val fmt = LocalMoney.current
    val line = syncLine(s.pc, s.syncing, s.connect, System.currentTimeMillis())
    DetailPage("Settings", nav::back) {
        Panel("Sync", glyph = "sync") {
            SettingRow("Your server", glyph = "globe", subtitle = line.text, onClick = nav::sync) { Dot(toneColor(line.tone), 7.dp) }
        }
        Panel("Look and money", glyph = "palette") {
            SettingRow("Appearance", glyph = "palette", subtitle = paletteName(s.settings.palette), onClick = nav::appearance)
            SettingRow("Currency and dates", glyph = "calendar", subtitle = formatSummary(s.settings.currency, s.settings.monthStartDay, s.settings.weekStartsMonday), onClick = nav::format)
        }
        Panel("Ledger", glyph = "list") {
            SettingRow("Accounts", glyph = "bank", subtitle = accountsSummary(s.accounts, fmt.formatWhole(s.net)), onClick = nav::accounts)
            SettingRow("Categories", glyph = "tag", subtitle = categoriesSummary(s.spendingCategories, s.incomeCategories), onClick = nav::categories)
            SettingRow("Import a statement", glyph = "download", subtitle = "Desjardins, Wealthsimple or any bank's CSV", onClick = { nav.import() })
            SettingRow("Backup", glyph = "save", subtitle = backupSummary(s.backup.auto, s.backup.lastAt.takeIf { it > 0 }?.let { backupWhen(it, s.today) }, s.backup.lastFailed), onClick = nav::backup)
            SettingRow("Export", glyph = "upload", subtitle = "Your entries as CSV, or the backup file", onClick = nav::export)
        }
        Panel("Data", glyph = "shield") {
            if (s.canLoadSample) {
                SettingRow("Try it with sample data", glyph = "sparkle", subtitle = "A sample household, labelled as one; erase removes it", onClick = data::loadSample)
            }
            SettingRow("Erase everything", glyph = "trash", titleColor = Tally.colors.heldText, subtitle = Copy.plural(s.entries, "entry", "entries") + " on this phone", onClick = { erasing = true })
            d.results[DataGroup.RESET]?.let { r -> T(resultText(r), Modifier.padding(4.dp), Tally.type.caption, if (r is DataResult.Failed) Tally.colors.heldText else Tally.colors.ink3) }
        }
        SettingRow("About Tally", glyph = "notes", value = "v" + s.version, onClick = nav::about)
    }
    if (erasing) {
        ConfirmDialog(
            title = "Erase everything?",
            body = "Every entry, account, budget, bill and goal on this phone goes. " + (if (s.pc.paired) "Your server keeps its copy: disconnect first if this phone should start from nothing. " else "") + "This cannot be undone.",
            confirm = "Erase",
            danger = true,
            onConfirm = { erasing = false; data.eraseAll() },
            onDismiss = { erasing = false },
        )
    }
}

internal fun resultText(r: DataResult): String = when (r) {
    is DataResult.Done -> r.message
    is DataResult.Failed -> r.message
}

internal fun paletteName(name: String): String = when (name) {
    "dark" -> "Dark"
    "oled" -> "Pure black"
    else -> "Matte"
}

// ── Sync ─────────────────────────────────────────────────────────────────────

@Composable
fun SyncRoute(nav: AppNav) {
    val vm: SettingsViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    SyncScreen(
        s,
        back = nav::back,
        syncNow = vm::syncNow,
        again = vm::connectAgain,
        phoneOnly = vm::setPhoneOnly,
        server = vm::setServer,
        send = vm::sendThisPhones,
        take = vm::takeServers,
        disconnect = vm::disconnect,
    )
}

/**
 * Where this phone stands with the owner's server, and the few things to do when it does not
 * stand well. Connecting asks the server by itself; the owner taps Allow on Discord once.
 */
@Composable
fun SyncScreen(
    s: SettingsState,
    back: () -> Unit,
    syncNow: () -> Unit,
    again: () -> Unit,
    phoneOnly: (Boolean) -> Unit,
    server: (String) -> Unit,
    send: () -> Unit,
    take: () -> Unit,
    disconnect: () -> Unit,
) {
    val c = Tally.colors
    val uri = LocalUriHandler.current
    val line = syncLine(s.pc, s.syncing, s.connect, System.currentTimeMillis())
    var address by rememberSaveable(s.server) { mutableStateOf(s.server) }
    var confirm by rememberSaveable { mutableStateOf<String?>(null) }
    DetailPage("Sync", back, subtitle = s.server.removePrefix("https://")) {
        Slab(Modifier.fillMaxWidth(), padding = PaddingValues(16.dp), spacing = 10.dp) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Dot(toneColor(line.tone), 9.dp)
                T(line.text, Modifier.weight(1f), Tally.type.title, c.ink, maxLines = 2)
            }
            line.detail?.let { T(it, style = Tally.type.ui, color = c.ink2) }
            val connect = s.connect
            if (!s.pc.paired && connect is Connect.Waiting) {
                Eyebrow("The code your bot shows")
                T(connect.code, style = Tally.type.hero.copy(fontSize = 44.sp, letterSpacing = 6.sp), color = c.signal)
                T("Tap Allow on the message from your bot on Discord. This phone connects by itself the moment you do, and syncs from then on with nothing to set up.", style = Tally.type.caption, color = c.ink3)
                Key("Open the dashboard instead", { runCatching { uri.openUri(connect.server + "/dashboard/tally") } }, glyph = "external", kind = if (connect.notified) KeyKind.Quiet else KeyKind.Plain, compact = true)
            }
            when {
                s.pc.paired && !s.pc.phoneOnly -> Key(if (s.syncing) "Syncing" else "Sync now", syncNow, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "sync", enabled = !s.syncing)
                connect == Connect.Denied || connect == Connect.Expired || connect is Connect.Trouble -> Key("Ask again", again, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "refresh")
                connect == Connect.Idle && !s.pc.phoneOnly -> Key("Connect now", syncNow, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "link")
            }
        }
        if (s.pc.paired) {
            Panel("Last sync", glyph = "history") {
                SettingRow("When", value = s.pc.lastSuccessAt.takeIf { it > 0 }?.let { com.tally.app.ui.shell.agoText(it, System.currentTimeMillis()) } ?: "Not yet")
                SettingRow("Sent and received", value = s.pc.lastSent.toString() + " · " + s.pc.lastReceived.toString())
                s.pc.lastError?.let { SettingRow("Problem", subtitle = it, titleColor = c.waiting) }
                s.pc.note?.let { SettingRow("News", subtitle = it) }
            }
        }
        Panel("How it works", glyph = "notes") {
            T(
                "Tally syncs with Relay's copy on your server, wherever the phone is: home Wi-Fi or not. Your PC's Relay syncs with the same copy, so all three hold one ledger. Entries made offline go when the network comes back; the newest edit wins.",
                Modifier.padding(4.dp),
                Tally.type.caption,
                c.ink3,
            )
        }
        Panel("Server", glyph = "globe") {
            Column(Modifier.padding(4.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Field(address, { address = it }, Modifier.fillMaxWidth(), placeholder = BuildConfig.TALLY_SERVER, leading = "globe", keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Done))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Key("Use this address", { server(address) }, enabled = address.isNotBlank() && address.trim() != s.server, compact = true)
                    Key("Default", { address = BuildConfig.TALLY_SERVER; server("") }, kind = KeyKind.Quiet, compact = true)
                }
            }
        }
        Panel("This phone", glyph = "device") {
            ToggleRow("Sync with my server", !s.pc.phoneOnly, { phoneOnly(!it) }, glyph = "sync", subtitle = if (s.pc.phoneOnly) "Off: the ledger stays on this phone" else "On: it syncs by itself")
            if (s.pc.paired) {
                SettingRow("Send this phone's ledger", glyph = "upload", subtitle = "Replace the server's copy with this phone's", onClick = { confirm = "send" })
                SettingRow("Take the server's ledger", glyph = "download", subtitle = "Replace this phone's with the server's copy", onClick = { confirm = "take" })
                SettingRow("Disconnect", glyph = "close", titleColor = c.heldText, subtitle = "Stop syncing; both copies stay as they are", onClick = disconnect)
            }
        }
    }
    when (confirm) {
        "send" -> ConfirmDialog("Send this phone's ledger?", "The server's copy, and your PC's on its next sync, become exactly what is on this phone. Anything only on the server goes.", "Send", { confirm = null; send() }, { confirm = null }, danger = true)
        "take" -> ConfirmDialog("Take the server's ledger?", "This phone's ledger becomes exactly the server's copy. Anything only on this phone goes.", "Take it", { confirm = null; take() }, { confirm = null }, danger = true)
    }
}

// ── Appearance ───────────────────────────────────────────────────────────────

@Composable
fun AppearanceRoute(nav: AppNav) {
    val vm: AppearanceViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    val c = Tally.colors
    DetailPage("Appearance", nav::back) {
        Labelled("Ground") {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Palette.ALL.forEach { p ->
                    Row(
                        Modifier.fillMaxWidth().clip(Radii.card).background(p.wall).border(1.dp, if (s.palette == p.name) c.ink else p.edge, Radii.card)
                            .clickable(role = Role.RadioButton) { vm.setPalette(p.name) }.padding(14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Box(Modifier.size(width = 44.dp, height = 28.dp).clip(Radii.key).background(p.slab).border(1.dp, p.strong, Radii.key))
                        Column(Modifier.weight(1f)) {
                            T(paletteName(p.name), style = Tally.type.ui, color = p.ink)
                            T(paletteHint(p.name), style = Tally.type.caption, color = p.ink3)
                        }
                        if (s.palette == p.name) Glyph("check", 16.dp, p.ink)
                    }
                }
            }
        }
        T("Relay's own grounds, so Tally sits beside it on the phone.", style = Tally.type.caption, color = c.ink3)
    }
}

internal fun paletteHint(name: String): String = when (name) {
    "dark" -> "A shade deeper"
    "oled" -> "True black, for an OLED screen"
    else -> "Relay's warm near-black"
}

// ── Currency and dates ───────────────────────────────────────────────────────

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun FormatRoute(nav: AppNav) {
    val vm: FormatViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    val c = Tally.colors
    val locale = LocalConfiguration.current.locales[0]
    DetailPage("Currency and dates", nav::back, subtitle = "This month: " + Dates.period(s.period, s.today)) {
        Labelled("Currency", hint = "Your server and PC take it on the next sync") {
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                currencyCodes(s.currency).forEach { code ->
                    val o = currencyOption(code, locale)
                    PickRow(o.name, s.currency == o.code, onClick = { vm.setCurrency(o.code) }, sub = o.code + " · " + o.subtitle)
                }
            }
        }
        Labelled("The month starts on", hint = "For a budget month that follows your pay day") {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                (1..BudgetPeriod.MAX_START_DAY).forEach { d -> Pill(ordinal(d), selected = s.monthStartDay == d, onClick = { vm.setMonthStartDay(d) }) }
            }
        }
        Labelled("The week starts on") {
            com.tally.app.ui.kit.Segmented(listOf(true to "Monday", false to "Sunday"), s.weekStartsMonday, vm::setWeekStartsMonday, Modifier.fillMaxWidth(), fill = true)
        }
        T("Amounts are kept to the cent; a switch to a currency with other decimals rewrites them to keep every figure.", style = Tally.type.caption, color = c.ink3)
    }
    s.pendingCurrency?.let { plan ->
        val o = currencyOption(plan.code, locale)
        ConfirmDialog("Switch to " + o.name + "?", roundingPrompt(o.name, plan.toDigits, plan.rounded), "Switch", vm::confirmCurrency, vm::cancelCurrency)
    }
}

// ── Backup ───────────────────────────────────────────────────────────────────

@Composable
fun BackupRoute(nav: AppNav) {
    val vm: BackupViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    val c = Tally.colors
    val folderPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri: Uri? -> if (uri != null) vm.setFolder(uri) }
    val fileSaver = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")) { uri: Uri? -> if (uri != null) vm.saveFile(uri) }
    val fileOpener = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? -> if (uri != null) vm.readFile(uri) }
    fun pick(launch: () -> Unit) { runCatching(launch).onFailure { vm.noPicker() } }
    DetailPage("Backup", nav::back, subtitle = Copy.plural(s.entries, "entry", "entries")) {
        Slab(padding = PaddingValues(horizontal = 10.dp, vertical = 4.dp)) {
            ToggleRow("Weekly backup", s.prefs.auto, vm::setAuto, glyph = "save", subtitle = backupSummary(s.prefs.auto, s.prefs.lastAt.takeIf { it > 0 }?.let { backupWhen(it, s.today) }, s.prefs.lastFailed))
            SettingRow(
                "A folder of your choice",
                glyph = "folder",
                subtitle = s.prefs.folderUri?.let { folderLabel(it) + if (s.prefs.folderFailed) " · the last copy there failed" else "" } ?: "A second copy that outlives an uninstall",
                onClick = { pick { folderPicker.launch(null) } },
            ) { if (s.prefs.folderUri != null) IconKey("close", vm::clearFolder, size = 14.dp, label = "Forget the folder") }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Key(s.busy?.working ?: "Back up now", vm::backupNow, Modifier.weight(1f), kind = KeyKind.Primary, glyph = "save", enabled = s.busy == null)
            Key("Save a file", { pick { fileSaver.launch(backupFileName(s.today)) } }, Modifier.weight(1f), glyph = "upload", enabled = s.busy == null)
        }
        s.result?.let { r -> T(resultText(r), style = Tally.type.caption, color = if (r is DataResult.Failed) c.heldText else c.ink3) }
        Panel("Kept on this phone", glyph = "history", subtitle = "Tap one to restore it") {
            if (s.copies.isEmpty()) T("No copies yet.", Modifier.padding(4.dp), Tally.type.caption, c.ink3)
            s.copies.forEach { copy ->
                ListRow(onClick = { vm.readCopy(copy) }, padding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)) {
                    Glyph("file", 16.dp, c.ink3)
                    T(backupWhen(copy.savedAt, s.today), Modifier.weight(1f), Tally.type.ui, c.ink)
                    T(sizeLabel(copy.bytes), style = Tally.type.mono, color = c.ink3)
                }
            }
        }
        Key("Restore from a file", { pick { fileOpener.launch(arrayOf("application/json", "*/*")) } }, Modifier.fillMaxWidth(), glyph = "download")
        T("A restore replaces this phone's ledger; the next sync then merges it with your server's.", style = Tally.type.caption, color = c.ink3)
    }
    s.pendingRestore?.let { p ->
        ConfirmDialog("Restore this backup?", restorePrompt(p.entries, p.savedOn?.let { Dates.short(it, s.today) }), "Restore", vm::confirmRestore, vm::cancelRestore, danger = true)
    }
}

// ── Export ───────────────────────────────────────────────────────────────────

@Composable
fun ExportRoute(nav: AppNav) {
    val vm: DataViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    val c = Tally.colors
    var monthOnly by rememberSaveable { mutableStateOf(false) }
    val csv = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("text/csv")) { uri: Uri? -> if (uri != null) vm.exportCsv(uri, if (monthOnly) s.period else null) }
    val file = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")) { uri: Uri? -> if (uri != null) vm.exportFile(uri) }
    DetailPage("Export", nav::back, subtitle = Copy.plural(s.entries, "entry", "entries")) {
        Panel("Entries as CSV", glyph = "file", subtitle = "For a spreadsheet; Tally reads it back too") {
            Column(Modifier.padding(4.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Key("Every entry", { monthOnly = false; runCatching { csv.launch(csvFileName(s.today)) }.onFailure { vm.noPicker(DataGroup.EXPORT) } }, Modifier.fillMaxWidth(), glyph = "upload", enabled = !s.isEmpty && s.busy == null)
                Key("This month only", { monthOnly = true; runCatching { csv.launch(monthCsvFileName(s.period.start)) }.onFailure { vm.noPicker(DataGroup.EXPORT) } }, Modifier.fillMaxWidth(), glyph = "calendar", enabled = !s.isEmpty && s.busy == null)
            }
        }
        Panel("The backup file", glyph = "save", subtitle = "Everything, as Tally restores it") {
            Key("Save the backup file", { runCatching { file.launch(backupFileName(s.today)) }.onFailure { vm.noPicker(DataGroup.EXPORT) } }, Modifier.fillMaxWidth().padding(4.dp), glyph = "upload", enabled = s.busy == null)
        }
        s.busy?.let { T(it.working, style = Tally.type.caption, color = c.ink3) }
        s.results[DataGroup.EXPORT]?.let { r -> T(resultText(r), style = Tally.type.caption, color = if (r is DataResult.Failed) c.heldText else c.ink3) }
    }
}

// ── About ────────────────────────────────────────────────────────────────────

@Composable
fun AboutRoute(nav: AppNav) {
    val c = Tally.colors
    DetailPage("About", nav::back) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            TallyMark(44.dp)
            Column {
                T("tally", style = Tally.type.brand.copy(fontSize = 26.sp), color = c.ink)
                T("Version " + BuildConfig.VERSION_NAME, style = Tally.type.mono, color = c.ink3)
            }
        }
        T("A month read as a pace: how much room is left, and whether the month is on track. In Relay's look, synced through Relay's copy on your own server.", style = Tally.type.ui, color = c.ink2)
        Hairline()
        SettingRow("Server", value = BuildConfig.TALLY_SERVER.removePrefix("https://"))
        SettingRow("Fonts", subtitle = "Geist and Geist Mono (Vercel), Sora: SIL Open Font License")
        SettingRow("Data", subtitle = "On this phone, on your server and on your PC. Nothing else: no account, no analytics.")
        Box(Modifier.height(8.dp))
    }
}
