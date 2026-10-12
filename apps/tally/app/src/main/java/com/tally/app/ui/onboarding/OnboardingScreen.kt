package com.tally.app.ui.onboarding

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.data.sync.Connect
import com.tally.app.ui.accounts.typeLabel
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.kit.Dot
import com.tally.app.ui.kit.Eyebrow
import com.tally.app.ui.kit.Field
import com.tally.app.ui.kit.Gap
import com.tally.app.ui.kit.IconKey
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.Labelled
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.PickRow
import com.tally.app.ui.kit.Pill
import com.tally.app.ui.kit.Slab
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.TallyMark
import com.tally.app.ui.kit.accountGlyph
import com.tally.app.ui.kit.column
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.settings.SettingsViewModel
import com.tally.app.ui.settings.currencyOption
import com.tally.app.ui.settings.onboardingCurrencyCodes
import com.tally.app.ui.shell.syncLine
import com.tally.app.ui.shell.toneColor
import com.tally.app.ui.theme.Tally
import com.tally.core.AccountType

@Composable
fun OnboardingRoute(nav: AppNav) {
    val vm: OnboardingViewModel = hiltViewModel()
    val s by vm.state.collectAsStateWithLifecycle()
    val sync: SettingsViewModel = hiltViewModel()
    val link by sync.state.collectAsStateWithLifecycle()
    var phase by rememberSaveable { mutableStateOf("connect") }
    LaunchedEffect(vm) { vm.done.collect { nav.finishOnboarding() } }
    // The server's ledger arrived on the first sync: nothing to ask.
    val tookLedger = link.loaded && link.pc.paired && link.pc.replaced && link.accounts > 0
    LaunchedEffect(tookLedger) { if (tookLedger && phase == "connect") vm.finishFromServer() }
    val fmt = LocalMoney.current
    if (phase == "connect") {
        ConnectStep(
            link,
            setUp = { phase = "setup" },
            again = sync::connectAgain,
            restore = nav::backup,
            sample = {
                // The sample is for trying the app on this phone: it never goes to the server.
                sync.setPhoneOnly(true)
                vm.trySample()
            },
        )
    } else {
        SetupSteps(
            s,
            back = { if (s.step == OnboardingStep.CURRENCY) phase = "connect" else vm.back() },
            pickCurrency = vm::pickCurrency,
            moreCurrencies = vm::toggleMoreCurrencies,
            name = vm::setName,
            type = vm::setType,
            balance = { vm.setBalance(it, fmt.parse(it)) },
            budget = { vm.setBudget(it, fmt.parse(it)) },
            addAccount = vm::addAccount,
            openNew = vm::openNewAccount,
            cancelNew = vm::cancelNewAccount,
            remove = vm::removeAccount,
            next = vm::next,
            skipBudget = vm::skipBudget,
        )
    }
}

/**
 * The first screen: Tally connecting to the owner's server by itself. The one thing it may need is
 * a tap on Allow in Discord; it says the code to look for. A server that already holds a ledger
 * (the first Tally's, the PC's) hands it over and the app opens on it.
 */
@Composable
private fun ConnectStep(link: com.tally.app.ui.settings.SettingsState, setUp: () -> Unit, again: () -> Unit, restore: () -> Unit, sample: () -> Unit) {
    val c = Tally.colors
    val uri = LocalUriHandler.current
    val line = syncLine(link.pc, link.syncing, link.connect, System.currentTimeMillis())
    val connect = link.connect
    val empty = link.pc.paired && link.pc.replaced && link.accounts == 0
    Box(Modifier.fillMaxSize().background(c.wall).statusBarsPadding().navigationBarsPadding(), contentAlignment = Alignment.TopCenter) {
        Column(
            Modifier.column(560.dp).fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 28.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                TallyMark(40.dp)
                T("tally", style = Tally.type.brand.copy(fontSize = 28.sp), color = c.ink)
            }
            T("Your budget, the same on this phone, your server and your PC.", style = Tally.type.heading, color = c.ink)
            T("Tally connects to the server your bot runs on by itself. Nothing to type, no link to paste.", style = Tally.type.ui, color = c.ink2)
            Gap(4.dp)
            Slab(Modifier.fillMaxWidth(), padding = PaddingValues(16.dp), spacing = 10.dp) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Dot(toneColor(line.tone), 9.dp)
                    T(
                        when {
                            empty -> "Connected. Your server has no ledger yet."
                            link.pc.paired -> "Connected. Bringing your ledger over."
                            else -> line.text
                        },
                        Modifier.weight(1f),
                        Tally.type.title,
                        c.ink,
                    )
                }
                if (!link.pc.paired && connect is Connect.Waiting) {
                    Eyebrow("Your phone's code")
                    T(connect.code, style = Tally.type.hero.copy(fontSize = 48.sp, letterSpacing = 8.sp), color = c.signal)
                    T(
                        if (connect.notified) "Your bot just sent you a message on Discord. Tap Allow if it shows the same code. That is the only step, once."
                        else "Your bot could not message you. Allow the request on its dashboard, Tally sync, where it shows the same code.",
                        style = Tally.type.ui,
                        color = c.ink2,
                    )
                    Key("Open the dashboard", { runCatching { uri.openUri(connect.server + "/dashboard/tally") } }, glyph = "external", kind = if (connect.notified) KeyKind.Quiet else KeyKind.Plain, compact = true)
                }
                line.detail?.takeIf { !link.pc.paired && connect !is Connect.Waiting }?.let { T(it, style = Tally.type.caption, color = c.ink3) }
                when {
                    empty -> Key("Set up your ledger", setUp, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "arrow-down")
                    connect == Connect.Denied || connect == Connect.Expired || connect is Connect.Trouble -> Key("Ask again", again, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "refresh")
                }
            }
            if (!link.pc.paired) {
                T("Or, while it waits:", style = Tally.type.caption, color = c.ink3)
                ListRow(onClick = setUp) {
                    IconKey("edit", setUp, label = null)
                    Column(Modifier.weight(1f)) {
                        T("Set up on this phone", style = Tally.type.ui, color = c.ink)
                        T("It still connects by itself and merges with your server", style = Tally.type.caption, color = c.ink3)
                    }
                }
                ListRow(onClick = restore) {
                    IconKey("download", restore, label = null)
                    Column(Modifier.weight(1f)) {
                        T("Restore a Tally backup", style = Tally.type.ui, color = c.ink)
                        T("The first Tally: Settings, Backup, Save a file", style = Tally.type.caption, color = c.ink3)
                    }
                }
                ListRow(onClick = sample) {
                    IconKey("sparkle", sample, label = null)
                    Column(Modifier.weight(1f)) {
                        T("Try it with sample data", style = Tally.type.ui, color = c.ink)
                        T("A labelled sample household, kept on this phone only", style = Tally.type.caption, color = c.ink3)
                    }
                }
            }
        }
    }
}

/** Currency, accounts, a monthly budget: the three questions, only for a ledger that starts here. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SetupSteps(
    s: OnboardingState,
    back: () -> Unit,
    pickCurrency: (String) -> Unit,
    moreCurrencies: () -> Unit,
    name: (String) -> Unit,
    type: (AccountType) -> Unit,
    balance: (String) -> Unit,
    budget: (String) -> Unit,
    addAccount: () -> Unit,
    openNew: () -> Unit,
    cancelNew: () -> Unit,
    remove: (Int) -> Unit,
    next: () -> Unit,
    skipBudget: () -> Unit,
) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    val locale = LocalConfiguration.current.locales[0]
    val index = OnboardingStep.entries.indexOf(s.step)
    Column(Modifier.fillMaxSize().background(c.wall).statusBarsPadding().imePadding()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            IconKey("arrow-left", back, tint = c.ink2, label = "Back")
            Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally)) {
                OnboardingStep.entries.forEachIndexed { i, _ -> Dot(if (i <= index) c.ink else c.track, 7.dp) }
            }
            Box(Modifier.padding(end = 44.dp))
        }
        Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
            Column(
                Modifier.column(560.dp).fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 12.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                T(s.step.question, style = Tally.type.heading, color = c.ink)
                T(s.step.caption, style = Tally.type.ui, color = c.ink3)
                when (s.step) {
                    OnboardingStep.CURRENCY -> {
                        onboardingCurrencyCodes(s.deviceCurrency, s.currency, s.moreCurrencies).forEach { code ->
                            val o = currencyOption(code, locale)
                            PickRow(o.name, s.currency == o.code, onClick = { pickCurrency(o.code) }, sub = o.code + " · " + o.subtitle)
                        }
                        Key(if (s.moreCurrencies) "Fewer currencies" else "More currencies", moreCurrencies, kind = KeyKind.Quiet, compact = true)
                    }
                    OnboardingStep.ACCOUNT -> {
                        s.accounts.forEachIndexed { i, a ->
                            ListRow {
                                IconKey(accountGlyph(a.type), {}, label = null)
                                Column(Modifier.weight(1f)) {
                                    T(a.name, style = Tally.type.ui, color = c.ink)
                                    T(typeLabel(a.type) + " · " + fmt.format(a.storedBalance), style = Tally.type.caption, color = c.ink3)
                                }
                                IconKey("close", { remove(i) }, size = 14.dp, label = "Remove")
                            }
                        }
                        if (s.formOpen) {
                            Slab(spacing = 10.dp) {
                                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                                    AccountType.entries.forEach { t -> Pill(typeLabel(t), selected = s.accountType == t, glyph = accountGlyph(t), onClick = { type(t) }) }
                                }
                                Labelled("Name") {
                                    Field(s.accountName, name, Modifier.fillMaxWidth(), keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words))
                                }
                                Labelled(if (s.accountType == AccountType.CREDIT) "What you owe on it today" else "What it holds today", problem = s.balanceProblem) {
                                    Field(s.balanceText, balance, Modifier.fillMaxWidth(), placeholder = fmt.format(0), leading = "coins", error = s.balanceProblem != null, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal))
                                }
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    Key("Add another", addAccount, glyph = "plus", compact = true, enabled = s.balanceProblem == null)
                                    if (s.accounts.isNotEmpty()) Key("Cancel", cancelNew, kind = KeyKind.Quiet, compact = true)
                                }
                            }
                        } else {
                            Key("Add an account", openNew, glyph = "plus", compact = true)
                        }
                        T("Together: " + fmt.format(s.net), style = Tally.type.caption, color = c.ink3)
                    }
                    OnboardingStep.BUDGET -> {
                        Field(s.budgetText, budget, Modifier.fillMaxWidth(), placeholder = fmt.format(0), leading = "pie", error = s.budgetProblem != null, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal))
                        s.budgetProblem?.let { T(it, style = Tally.type.caption, color = c.heldText) }
                        T("Without one, Tally reads the month against what came in.", style = Tally.type.caption, color = c.ink3)
                    }
                }
                s.error?.let { T(it, style = Tally.type.ui, color = c.heldText, align = TextAlign.Start) }
            }
        }
        Column(Modifier.fillMaxWidth().background(c.console).navigationBarsPadding().padding(16.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (s.step == OnboardingStep.BUDGET) Key("Skip", skipBudget, Modifier.weight(1f), kind = KeyKind.Quiet)
                Key(if (s.step == OnboardingStep.BUDGET) "Start" else "Continue", next, Modifier.weight(2f), kind = KeyKind.Primary, enabled = s.canContinue, glyph = if (s.step == OnboardingStep.BUDGET) "check" else "chevron-right")
            }
        }
    }
}
