package com.tally.app.ui.banks

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.plaid.link.OpenPlaidLink
import com.plaid.link.Plaid
import com.plaid.link.configuration.LinkTokenConfiguration
import com.plaid.link.result.LinkExit
import com.plaid.link.result.LinkSuccess
import com.tally.app.data.banks.Bank
import com.tally.app.data.banks.BankAccount
import com.tally.app.data.banks.BanksAnswer
import com.tally.app.data.db.AccountEntity
import com.tally.app.ui.kit.ConfirmDialog
import com.tally.app.ui.kit.DetailPage
import com.tally.app.ui.kit.Glyph
import com.tally.app.ui.kit.GlyphTile
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.PickRow
import com.tally.app.ui.kit.Pill
import com.tally.app.ui.kit.Postcard
import com.tally.app.ui.kit.Radii
import com.tally.app.ui.kit.Scene
import com.tally.app.ui.kit.Sheet
import com.tally.app.ui.kit.Slab
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.Toggle
import com.tally.app.ui.kit.ago
import com.tally.app.ui.kit.softOf
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.theme.Hue
import com.tally.app.ui.theme.Tally

@Composable
fun BanksRoute(nav: AppNav, vm: BanksViewModel = hiltViewModel()) {
    val s by vm.state.collectAsStateWithLifecycle()
    val accounts by vm.accounts.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val launcher = rememberLauncherForActivityResult(OpenPlaidLink()) { result ->
        when (result) {
            is LinkSuccess -> vm.linked(result.publicToken, result.metadata.institution?.id, result.metadata.institution?.name)
            is LinkExit -> vm.linkClosed(result.error?.displayMessage ?: result.error?.errorMessage)
            else -> vm.linkClosed(null)
        }
    }
    val openLink: () -> Unit = {
        vm.startLink { token ->
            val session = Plaid.createPlaidLinkSession(context.applicationContext as android.app.Application, LinkTokenConfiguration.Builder().token(token).build())
            launcher.launch(session)
        }
    }
    BanksScreen(
        s, accounts,
        BanksActions(
            back = nav::back, connect = openLink, sync = nav::sync, reload = vm::reload, toggle = vm::toggle, into = vm::into,
            since = vm::since, follow = vm::follow, cancel = vm::cancelChoosing, edit = vm::edit, refresh = vm::refresh,
            remove = vm::remove, dismiss = vm::dismissNotice,
        ),
    )
}

data class BanksActions(
    val back: () -> Unit = {},
    val connect: () -> Unit = {},
    val sync: () -> Unit = {},
    val reload: () -> Unit = {},
    val toggle: (String) -> Unit = {},
    val into: (String, String?) -> Unit = { _, _ -> },
    val since: (Since) -> Unit = {},
    val follow: () -> Unit = {},
    val cancel: () -> Unit = {},
    val edit: (Bank) -> Unit = {},
    val refresh: () -> Unit = {},
    val remove: (Bank) -> Unit = {},
    val dismiss: () -> Unit = {},
)

@Composable
fun BanksScreen(s: BanksState, accounts: List<AccountEntity>, a: BanksActions) {
    val choosing = s.choosing
    if (choosing != null) {
        ChooseAccounts(choosing, s, accounts, a)
        return
    }
    val banks = (s.answer as? BanksAnswer.Ready)?.banks.orEmpty()
    DetailPage(
        "Bank connections",
        onBack = a.back,
        subtitle = "Through Plaid, read only",
        footer = if (s.answer is BanksAnswer.Ready) ({
            Key(if (banks.isEmpty()) "Connect a bank" else "Connect another bank", a.connect, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "link", enabled = s.working == null)
        }) else null,
    ) {
        Status(s, a)
        when (val answer = s.answer) {
            null -> T("Asking your server", style = Tally.type.caption, color = Tally.colors.ink3)
            BanksAnswer.NotPaired -> Postcard(Scene.BANKS, sceneHeight = 104.dp) {
                T("Connect this phone to your server first", style = Tally.type.title)
                T("Bank connections live on your server, so the phone asks it to open Plaid. Once this phone syncs, come back here.", style = Tally.type.body.copy(fontSize = 14.sp), color = Tally.colors.ink2)
                Key("Open sync", a.sync, Modifier.fillMaxWidth(), kind = KeyKind.Primary)
            }
            BanksAnswer.NotOnServer -> Postcard(Scene.BANKS, sceneHeight = 104.dp) {
                T("Your server does not connect banks yet", style = Tally.type.title)
                T("Tally is ready for Plaid. Your server's Relay needs the bank update and your Plaid keys; docs/PLAID.md in Tally's repository says what it takes.", style = Tally.type.body.copy(fontSize = 14.sp), color = Tally.colors.ink2)
                Key("Ask again", a.reload, Modifier.fillMaxWidth())
            }
            is BanksAnswer.Failed -> Slab(Modifier.fillMaxWidth(), spacing = 8.dp) {
                T("Your server did not answer", style = Tally.type.title)
                T(answer.reason, style = Tally.type.caption, color = Tally.colors.ink3)
                Key("Try again", a.reload, Modifier.fillMaxWidth())
            }
            is BanksAnswer.Ready -> {
                if (answer.banks.isEmpty()) Welcome(a) else answer.banks.forEach { BankCard(it, accounts, a) }
                if (answer.banks.isNotEmpty()) Key("Check for new purchases now", a.refresh, Modifier.fillMaxWidth(), glyph = "sync", enabled = s.working == null)
                Trust()
            }
        }
    }
}

@Composable
private fun Status(s: BanksState, a: BanksActions) {
    val c = Tally.colors
    val line = s.working ?: s.problem ?: s.notice ?: return
    val hue = when {
        s.working != null -> Hue.sky
        s.problem != null -> Hue.coral
        else -> Hue.mint
    }
    Row(
        Modifier.fillMaxWidth().clip(Radii.key).background(softOf(hue)).clickable(enabled = s.working == null, role = Role.Button, onClick = a.dismiss).padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Glyph(if (s.working != null) "sync" else if (s.problem != null) "close" else "check", 16.dp, com.tally.app.ui.kit.deepOf(hue))
        T(line, Modifier.weight(1f), Tally.type.ui, com.tally.app.ui.kit.deepOf(hue))
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Welcome(a: BanksActions) {
    val c = Tally.colors
    Postcard(Scene.BANKS, sceneHeight = 112.dp) {
        T("Stop importing CSVs", style = Tally.type.title.copy(fontSize = 19.sp))
        T("Connect Desjardins and Wealthsimple once. New purchases land in Tally on their own, already sorted.", style = Tally.type.body.copy(fontSize = 14.sp), color = c.ink2)
    }
    T("Yours", style = Tally.type.eyebrow, color = c.ink3)
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        BankTile("Desjardins", "AccèsD sign-in", Modifier.weight(1f), a.connect)
        BankTile("Wealthsimple", "Cash, TFSA, RRSP", Modifier.weight(1f), a.connect)
    }
    T("Other banks", style = Tally.type.eyebrow, color = c.ink3)
    FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        listOf("RBC", "TD", "National Bank", "BMO", "Scotiabank", "Tangerine").forEach { Pill(it, onClick = a.connect, color = c.ink) }
    }
}

@Composable
private fun BankTile(name: String, sub: String, modifier: Modifier, onClick: () -> Unit) {
    val (fill, _) = bankLook(name)
    Column(
        modifier.height(128.dp).clip(Radii.card).background(softOf(fill)).clickable(role = Role.Button, onClick = onClick).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp, Alignment.Bottom),
    ) {
        BankMark(name, size = 44.dp)
        T(name, style = Tally.type.ui, weight = FontWeight.SemiBold)
        T(sub, style = Tally.type.caption, color = Tally.colors.ink3, maxLines = 1)
    }
}

@Composable
private fun Trust() {
    Row(
        Modifier.fillMaxWidth().clip(Radii.card).background(softOf(Hue.sky)).padding(14.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Glyph("lock", 18.dp, Hue.skyInk)
        T(
            "You sign in on your bank's own page through Plaid. Your password never reaches Tally or your server, Plaid's access is read only, and you can disconnect any time.",
            Modifier.weight(1f), Tally.type.caption.copy(fontSize = 13.sp, lineHeight = 18.sp), Hue.skyInk,
        )
    }
}

@Composable
private fun BankCard(bank: Bank, accounts: List<AccountEntity>, a: BanksActions) {
    val c = Tally.colors
    var confirm by remember { mutableStateOf(false) }
    Slab(Modifier.fillMaxWidth(), spacing = 12.dp, onClick = { a.edit(bank) }) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            BankMark(bank.institution)
            Column(Modifier.weight(1f)) {
                T(bank.institution, style = Tally.type.title)
                val followed = bank.accounts.count { it.follow }
                T(
                    "$followed of ${bank.accounts.size} accounts followed" + (bank.lastSync?.let { " · read " + ago(it) } ?: " · not read yet"),
                    style = Tally.type.caption, color = c.ink3, maxLines = 2,
                )
            }
            Box(Modifier.size(10.dp).clip(CircleShape).background(if (bank.problem == null) c.live else c.waiting))
        }
        bank.problem?.let { T(it, style = Tally.type.caption, color = c.waitingText) }
        bank.accounts.filter { it.follow }.forEach { acc ->
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                GlyphTile(if ("credit" in acc.kind.lowercase()) "card" else "bank", Hue.violet, size = 30.dp)
                Column(Modifier.weight(1f)) {
                    T(acc.name + mask(acc), style = Tally.type.ui, maxLines = 1)
                    T("Into " + (accounts.firstOrNull { it.uid == acc.into }?.name ?: "a new account"), style = Tally.type.caption, color = c.ink3, maxLines = 1)
                }
            }
        }
        T("Disconnect " + bank.institution, Modifier.clip(Radii.key).clickable(role = Role.Button) { confirm = true }.padding(vertical = 6.dp), Tally.type.uiMedium, c.heldText, weight = FontWeight.SemiBold)
    }
    if (confirm) {
        ConfirmDialog(
            "Disconnect ${bank.institution}?",
            "Your server stops reading it and forgets its access. Entries it brought in stay in your ledger.",
            "Disconnect",
            onConfirm = { confirm = false; a.remove(bank) },
            onDismiss = { confirm = false },
            danger = true,
        )
    }
}

private fun mask(a: BankAccount) = if (a.mask.isNotBlank()) " •• " + a.mask else ""

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ChooseAccounts(bank: Bank, s: BanksState, accounts: List<AccountEntity>, a: BanksActions) {
    val c = Tally.colors
    var picking by remember { mutableStateOf<BankAccount?>(null) }
    val followed = bank.accounts.count { it.follow }
    DetailPage(
        bank.institution,
        onBack = a.cancel,
        subtitle = "Pick what Tally follows",
        footer = {
            Key(
                if (followed == 0) "Follow nothing for now" else "Follow $followed account" + if (followed == 1) "" else "s",
                a.follow, Modifier.fillMaxWidth(), kind = KeyKind.Primary, enabled = s.working == null,
            )
        },
    ) {
        Status(s, a)
        Column(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Box(Modifier.size(76.dp).shadow(12.dp, CircleShape, spotColor = Hue.mint).clip(CircleShape).background(Hue.mint), contentAlignment = Alignment.Center) {
                Glyph("check", 34.dp, Color.White)
            }
            T(bank.institution + " is connected", style = Tally.type.heading.copy(fontSize = 23.sp), align = TextAlign.Center)
            T(
                "Tally found ${bank.accounts.size} account" + (if (bank.accounts.size == 1) "" else "s") + ". Pick the ones you pay with; the rest stay out.",
                style = Tally.type.body.copy(fontSize = 14.sp), color = c.ink2, align = TextAlign.Center,
            )
        }
        Slab(Modifier.fillMaxWidth(), spacing = 14.dp) {
            bank.accounts.forEach { acc ->
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        T(acc.name + mask(acc), style = Tally.type.ui, weight = FontWeight.SemiBold, maxLines = 1)
                        val target = accounts.firstOrNull { it.uid == acc.into }
                        T(
                            if (!acc.follow) acc.kind.ifBlank { "Left out" }.replaceFirstChar { it.uppercase() } + " · left out"
                            else "Into " + (target?.name ?: "a new account"),
                            Modifier.then(if (acc.follow) Modifier.clickable(role = Role.Button) { picking = acc } else Modifier),
                            Tally.type.caption, if (acc.follow) c.signal else c.ink3, maxLines = 1,
                        )
                    }
                    Toggle(acc.follow, { a.toggle(acc.id) })
                }
            }
        }
        T("Bring in", style = Tally.type.eyebrow, color = c.ink3)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Since.entries.forEach { Pill(it.label, selected = s.since == it, onClick = { a.since(it) }, color = c.ink) }
        }
        Row(
            Modifier.fillMaxWidth().clip(Radii.card).background(softOf(Hue.sun)).padding(14.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Glyph("history", 18.dp, Hue.sunInk)
            T("Entries you already have, from a CSV or by hand, are matched and skipped.", Modifier.weight(1f), Tally.type.caption.copy(fontSize = 13.sp), Hue.sunInk)
        }
    }
    picking?.let { acc ->
        Sheet(onDismiss = { picking = null }, title = "Where " + acc.name + " lands") {
            PickRow("A new account", acc.into == null, { a.into(acc.id, null); picking = null }, sub = "Named " + acc.name)
            accounts.filter { !it.archived }.forEach { t ->
                PickRow(t.name, acc.into == t.uid, { a.into(acc.id, t.uid); picking = null }, sub = t.type.name.lowercase().replaceFirstChar { it.uppercase() })
            }
        }
    }
}
