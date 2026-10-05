package com.tally.app.ui.onboarding

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.AccountBalanceWallet
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.CalendarToday
import androidx.compose.material.icons.rounded.EditNote
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material.icons.rounded.Language
import androidx.compose.material.icons.rounded.Payments
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.ui.common.Caption
import com.tally.app.ui.common.CategoryIcons
import com.tally.app.ui.common.ChromeButton
import com.tally.app.ui.common.Dates
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.Group
import com.tally.app.ui.common.GroupFooter
import com.tally.app.ui.common.GroupHeader
import com.tally.app.ui.common.GroupRow
import com.tally.app.ui.common.HeroAction
import com.tally.app.ui.common.HeroPanel
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.PANEL_GAP
import com.tally.app.ui.common.PaceMeter
import com.tally.app.ui.common.ROW_PAD
import com.tally.app.ui.common.RowPill
import com.tally.app.ui.common.SecondaryAction
import com.tally.app.ui.common.StatChip
import com.tally.app.ui.common.TextAction
import com.tally.app.ui.common.bounceClick
import com.tally.app.ui.common.selectableColors
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.settings.COMMON_CURRENCIES
import com.tally.app.ui.settings.CurrencyRow
import com.tally.app.ui.settings.HeroHead
import com.tally.app.ui.settings.HeroNumber
import com.tally.app.ui.settings.RowBadge
import com.tally.app.ui.settings.currencyOption
import com.tally.app.ui.settings.onboardingCurrencyCodes
import com.tally.app.ui.theme.TallyMotion
import com.tally.core.AccountType
import com.tally.core.Copy
import com.tally.core.PaceReading

@Composable
fun OnboardingRoute(nav: AppNav) {
    val viewModel: OnboardingViewModel = hiltViewModel()
    val state by viewModel.state.collectAsStateWithLifecycle()
    LaunchedEffect(viewModel) { viewModel.done.collect { nav.finishOnboarding() } }
    // System Back walks the steps; on the first one it leaves the app as usual.
    BackHandler(enabled = state.step != OnboardingStep.CURRENCY) { viewModel.back() }
    val actions = remember(viewModel) {
        OnboardingActions(
            back = viewModel::back,
            next = viewModel::next,
            skip = viewModel::skipBudget,
            trySample = viewModel::trySample,
            pickCurrency = viewModel::pickCurrency,
            toggleMore = viewModel::toggleMoreCurrencies,
            setName = viewModel::setName,
            setType = viewModel::setType,
            setBalance = viewModel::setBalance,
            setBudget = viewModel::setBudget,
            addAccount = viewModel::addAccount,
            openNewAccount = viewModel::openNewAccount,
            cancelNewAccount = viewModel::cancelNewAccount,
            removeAccount = viewModel::removeAccount,
        )
    }
    OnboardingScreen(state, actions)
}

/** Everything first run can do, as plain lambdas. */
data class OnboardingActions(
    val back: () -> Unit = {},
    val next: () -> Unit = {},
    val skip: () -> Unit = {},
    val trySample: () -> Unit = {},
    val pickCurrency: (String) -> Unit = {},
    val toggleMore: () -> Unit = {},
    val setName: (String) -> Unit = {},
    val setType: (AccountType) -> Unit = {},
    val setBalance: (String, Long?) -> Unit = { _, _ -> },
    val setBudget: (String, Long?) -> Unit = { _, _ -> },
    /** Moves the form's account into the list. */
    val addAccount: () -> Unit = {},
    /** Opens the form for one more account. */
    val openNewAccount: () -> Unit = {},
    val cancelNewAccount: () -> Unit = {},
    /** Takes the account at this place in the list back out. */
    val removeAccount: (Int) -> Unit = {},
)

/**
 * First run: a step rail, one question per step with the thing it builds drawn under it in the lit
 * panel, and the step's one act at the bottom under the thumb.
 */
@Composable
fun OnboardingScreen(state: OnboardingState, actions: OnboardingActions) {
    // Forward slides in from the end, back from the start, in either reading direction.
    val dir = if (LocalLayoutDirection.current == LayoutDirection.Rtl) -1 else 1
    Column(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().imePadding()) {
        StepHeader(state, actions)
        AnimatedContent(
            targetState = state.step,
            modifier = Modifier.weight(1f).fillMaxWidth(),
            transitionSpec = {
                val forward = if (targetState.ordinal > initialState.ordinal) dir else -dir
                (slideInHorizontally(TallyMotion.enter(TallyMotion.Emphasized)) { w -> forward * w / 6 } + fadeIn(TallyMotion.enter()))
                    .togetherWith(slideOutHorizontally(TallyMotion.exit()) { w -> -forward * w / 6 } + fadeOut(TallyMotion.exit(TallyMotion.Fast)))
            },
            label = "onboarding_step",
        ) { step ->
            Column(
                Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState())
                    .padding(start = GUTTER, end = GUTTER, top = 12.dp, bottom = 16.dp),
                verticalArrangement = Arrangement.spacedBy(PANEL_GAP),
            ) {
                Question(step)
                when (step) {
                    OnboardingStep.CURRENCY -> CurrencyStep(state, actions)
                    OnboardingStep.ACCOUNT -> AccountStep(state, actions)
                    OnboardingStep.BUDGET -> BudgetStep(state, actions)
                }
            }
        }
        BottomActions(state, actions)
    }
}

/** Back (from step 2), the rail of steps, and where the step sits or its skip. */
@Composable
private fun StepHeader(state: OnboardingState, actions: OnboardingActions) {
    val count = OnboardingStep.entries.size
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp)
            .heightIn(min = 48.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (state.step != OnboardingStep.CURRENCY) {
            ChromeButton(Icons.AutoMirrored.Rounded.ArrowBack, "Back", actions.back)
        } else {
            Spacer(Modifier.size(48.dp))
        }
        StepRail(state.step.ordinal, count, Modifier.weight(1f))
        if (state.step == OnboardingStep.BUDGET) {
            TextAction("skip", actions.skip)
        } else {
            Text(
                "${state.step.ordinal + 1} OF $count",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.clearAndSetSemantics { },
            )
        }
    }
}

/** One rounded bar per step: the accent for done and current, the raised rung for what is ahead. */
@Composable
private fun StepRail(current: Int, count: Int, modifier: Modifier = Modifier) {
    Row(
        modifier.semantics { contentDescription = "Step ${current + 1} of $count" },
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        repeat(count) { i ->
            val color by animateColorAsState(
                if (i <= current) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceContainerHigh,
                TallyMotion.standard(),
                label = "rail_$i",
            )
            Box(
                Modifier
                    .weight(1f)
                    .height(4.dp)
                    .clip(RoundedCornerShape(4.dp))
                    .background(color)
            )
        }
    }
}

@Composable
private fun Question(step: OnboardingStep) {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(
            step.question,
            style = MaterialTheme.typography.headlineMedium,
            color = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.semantics { heading() },
        )
        Text(step.caption, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** The step's one act, full width; the sample shortcut sits under it on the first step. */
@Composable
private fun BottomActions(state: OnboardingState, actions: OnboardingActions) {
    Column(
        Modifier.fillMaxWidth().padding(start = GUTTER, end = GUTTER, top = 8.dp, bottom = 12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        val error = state.error
        if (error != null) {
            Text(
                error,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
                textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth(),
            )
        }
        HeroAction(
            if (state.step == OnboardingStep.BUDGET) "Start" else "Continue",
            actions.next,
            Modifier.fillMaxWidth(),
            enabled = state.canContinue,
        )
        if (state.step == OnboardingStep.CURRENCY) {
            TextAction("try it with sample data", actions.trySample, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

// ── Step 1: currency ─────────────────────────────────────────────────────────

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun CurrencyStep(state: OnboardingState, actions: OnboardingActions) {
    val locale = LocalMoney.current.locale
    val picked = remember(state.currency, locale) { currencyOption(state.currency, locale) }
    HeroPanel {
        HeroHead("Your currency", end = picked.code)
        Spacer(Modifier.height(12.dp))
        HeroNumber(picked.sample, description = "Amounts will read like ${picked.sample}")
        Text(
            picked.name + " · " + picked.decimalsLine.replaceFirstChar { it.lowercase() },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            StatChip(Icons.Rounded.PhoneAndroid, "This phone uses ${state.deviceCurrency}")
            StatChip(Icons.Rounded.Tune, "Change it in Settings")
        }
    }
    val codes = remember(state.deviceCurrency, state.currency, state.moreCurrencies) {
        onboardingCurrencyCodes(state.deviceCurrency, state.currency, state.moreCurrencies)
    }
    val options = remember(codes, locale) { codes.map { currencyOption(it, locale) } }
    val rows = options.map { option ->
        val row: @Composable (Shape) -> Unit = { shape ->
            CurrencyRow(option, selected = option.code == state.currency, shape = shape) { actions.pickCurrency(option.code) }
        }
        row
    }
    val hidden = remember(codes) { COMMON_CURRENCIES.filter { it !in codes } }
    val more: @Composable (Shape) -> Unit = { shape ->
        GroupRow(
            "More currencies",
            shape,
            subtitle = hidden.take(3).joinToString(", ") + if (hidden.size > 3) " and ${hidden.size - 3} more" else "",
            leading = { RowBadge(Icons.Rounded.Language) },
            trailing = { Icon(Icons.Rounded.ExpandMore, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant) },
            chevron = false,
            onClick = actions.toggleMore,
        )
    }
    Group(
        rows = if (state.moreCurrencies || hidden.isEmpty()) rows else rows + more,
        modifier = Modifier.padding(top = 6.dp),
        title = "Currency",
        trailing = state.currency,
    )
}

// ── Step 2: the accounts ─────────────────────────────────────────────────────

/**
 * The accounts being built, open under the question: with one in the form, its balance; with
 * several, their net. Then the accounts added so far, each one tap from removed, and the form for
 * the next. Continue adds an open form's account too, so one account takes no extra tap.
 */
@Composable
private fun AccountStep(state: OnboardingState, actions: OnboardingActions) {
    val money = LocalMoney.current
    val credit = state.accountType == AccountType.CREDIT
    val typeName = defaultAccountName(state.accountType)
    val several = state.accounts.isNotEmpty()
    HeroPanel {
        if (several) {
            val all = state.toCreate
            HeroHead("Your accounts", end = Copy.plural(all.size, "account").uppercase())
            Spacer(Modifier.height(12.dp))
            HeroNumber(money.format(state.net), description = "Net across your accounts, " + money.format(state.net))
            Text(
                "net across " + accountNames(all),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            HeroHead("First account", end = typeName.uppercase())
            Spacer(Modifier.height(12.dp))
            HeroNumber(
                money.format(state.storedBalance),
                color = if ((state.balance ?: 0L) > 0L) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                description = "Starting balance, " + money.format(state.storedBalance),
            )
            Text(
                accountNameToSave(state.accountName, state.accountType) + " · " + if (credit) "owed today" else "balance today",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
    if (several) {
        val rows: List<@Composable (Shape) -> Unit> = state.accounts.mapIndexed { i, a -> addedRow(a, i, actions) } +
            if (state.formOpen) emptyList() else listOf(anotherRow(actions))
        Group(rows = rows, modifier = Modifier.padding(top = 6.dp), title = "Added", trailing = Copy.plural(state.accounts.size, "account"))
    }
    if (state.formOpen) {
        Column(Modifier.fillMaxWidth().padding(top = 6.dp)) {
            GroupHeader(if (several) "Next account" else "Name")
            OnboardField(
                value = state.accountName,
                onChange = actions.setName,
                label = "Account name",
                placeholder = typeName,
                icon = Icons.Rounded.EditNote,
                capitalization = KeyboardCapitalization.Words,
            )
        }
        Column(Modifier.fillMaxWidth()) {
            GroupHeader("Type")
            TypeTiles(state.accountType, actions.setType)
        }
        Column(Modifier.fillMaxWidth()) {
            GroupHeader(if (credit) "Owed on it today" else "Balance today", trailing = money.currency.currencyCode)
            OnboardField(
                value = state.balanceText,
                onChange = { text -> actions.setBalance(text, money.parse(text)) },
                label = if (credit) "Amount owed" else "Starting balance",
                placeholder = money.formatInput(0L),
                icon = Icons.Rounded.Payments,
                keyboardType = if (money.fractionDigits > 0) KeyboardType.Decimal else KeyboardType.Number,
                suffix = money.symbol,
                isError = state.balanceProblem != null,
            )
            val problem = state.balanceProblem
            if (problem != null) GroupFooter(problem, isError = true)
        }
        Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
            SecondaryAction(
                "Add this account",
                actions.addAccount,
                Modifier.fillMaxWidth(),
                enabled = state.balanceProblem == null && !state.busy,
            )
            if (several) TextAction("cancel", actions.cancelNewAccount, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/** One added account: its kind, name and balance; the whole row takes it back out. */
private fun addedRow(a: OnboardAccount, index: Int, actions: OnboardingActions): @Composable (Shape) -> Unit = { shape ->
    val money = LocalMoney.current
    GroupRow(
        a.name,
        shape,
        subtitle = defaultAccountName(a.type) + " · " + money.format(a.storedBalance),
        leading = { GlyphBadge(CategoryIcons.account(a.type)) },
        trailing = { RowPill("Remove") },
        chevron = false,
        onClick = { actions.removeAccount(index) },
    )
}

/** The row that opens the form for one more account. */
private fun anotherRow(actions: OnboardingActions): @Composable (Shape) -> Unit = { shape ->
    GroupRow(
        "Add another account",
        shape,
        subtitle = "A card, savings, cash or investments",
        leading = { RowBadge(Icons.Rounded.Add) },
        chevron = false,
        onClick = actions.openNewAccount,
    )
}

/** The four account kinds as tiles; one radio group. Two a row once the font is large. */
@Composable
private fun TypeTiles(selected: AccountType, onPick: (AccountType) -> Unit) {
    val columns = if (LocalDensity.current.fontScale > 1.3f) 2 else 4
    val rows = remember(columns) { AccountType.entries.chunked(columns) }
    Column(Modifier.fillMaxWidth().selectableGroup(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        rows.forEach { row ->
            Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                row.forEach { type ->
                    TypeTile(type, type == selected, Modifier.weight(1f).fillMaxHeight()) { onPick(type) }
                }
            }
        }
    }
}

@Composable
private fun TypeTile(type: AccountType, picked: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val (border, fill) = selectableColors(picked)
    val shape = RoundedCornerShape(14.dp)
    val label = defaultAccountName(type)
    Column(
        modifier
            .clip(shape)
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .background(fill)
            .border(if (picked) 1.5.dp else 1.dp, border, shape)
            .bounceClick(label = label, role = Role.RadioButton, onClick = onClick)
            .semantics { selected = picked }
            .padding(horizontal = 4.dp, vertical = 12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        GlyphBadge(
            CategoryIcons.account(type),
            tint = if (picked) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onBackground,
            size = 40.dp,
        )
        Text(
            label,
            style = MaterialTheme.typography.bodySmall,
            color = if (picked) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

// ── Step 3: the month's budget ───────────────────────────────────────────────

/** Under the question, the thing being built: Home's hero at zero spent, with the typed budget as its figure. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun BudgetStep(state: OnboardingState, actions: OnboardingActions) {
    val money = LocalMoney.current
    val budget = state.budget ?: 0L
    val reading = remember(budget, state.period, state.today) {
        PaceReading(budget, 0L, state.period.days, state.period.elapsedDays(state.today))
    }
    HeroPanel {
        HeroHead("Left to spend", end = Dates.period(state.period, state.today).uppercase())
        Spacer(Modifier.height(12.dp))
        HeroNumber(
            money.formatWhole(budget),
            color = if (budget > 0L) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
            description = "Monthly budget, " + money.formatWhole(budget),
        )
        Text(
            if (budget > 0L) "to spend this month, none of it spent yet" else "Type it below, or skip and Home reads the month against income",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))
        PaceMeter(
            0f,
            if (budget > 0L) reading.paceFraction else null,
            if (budget > 0L) {
                "Nothing spent of ${money.formatWhole(budget)}. An even pace would be ${money.formatWhole(reading.expected)} by today."
            } else {
                "No budget typed yet"
            },
            height = 14.dp,
        )
        Spacer(Modifier.height(8.dp))
        Caption("Home counts down from this figure")
        Spacer(Modifier.height(14.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            // Home's allowance chip, in its words ("$94 a day for 31 days"); with no budget yet,
            // only the days it would cover.
            if (budget > 0L) {
                StatChip(Icons.Rounded.Speed, Copy.marginLine(reading, money))
            } else {
                StatChip(Icons.Rounded.CalendarToday, Copy.plural(reading.daysLeft, "day") + " left")
            }
        }
    }
    Column(Modifier.fillMaxWidth().padding(top = 6.dp)) {
        GroupHeader("Monthly budget", trailing = money.currency.currencyCode)
        OnboardField(
            value = state.budgetText,
            onChange = { text -> actions.setBudget(text, money.parse(text)) },
            label = "Monthly budget",
            placeholder = money.formatInput(0L),
            icon = Icons.Rounded.AccountBalanceWallet,
            keyboardType = if (money.fractionDigits > 0) KeyboardType.Decimal else KeyboardType.Number,
            suffix = money.symbol,
            isError = state.budgetProblem != null,
            imeAction = ImeAction.Go,
            onIme = actions.next,
        )
        val problem = state.budgetProblem
        if (problem != null) GroupFooter(problem, isError = true)
    }
}

/**
 * A filled rounded field with its glyph, the search-field look from Avex. [label] is what TalkBack
 * reads; [placeholder] shows while it is empty. The reason for an error is a line under it.
 */
@Composable
private fun OnboardField(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    placeholder: String,
    icon: ImageVector,
    modifier: Modifier = Modifier,
    keyboardType: KeyboardType = KeyboardType.Text,
    capitalization: KeyboardCapitalization = KeyboardCapitalization.None,
    suffix: String? = null,
    isError: Boolean = false,
    imeAction: ImeAction = ImeAction.Done,
    onIme: (() -> Unit)? = null,
) {
    val focus = LocalFocusManager.current
    val shape = RoundedCornerShape(16.dp)
    val edge = MaterialTheme.colorScheme.error
    BasicTextField(
        value = value,
        onValueChange = onChange,
        modifier = modifier
            .fillMaxWidth()
            .semantics { contentDescription = label },
        singleLine = true,
        textStyle = MaterialTheme.typography.bodyLarge.copy(color = MaterialTheme.colorScheme.onBackground),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        keyboardOptions = KeyboardOptions(
            capitalization = capitalization,
            keyboardType = keyboardType,
            imeAction = imeAction,
        ),
        keyboardActions = KeyboardActions(
            onDone = { focus.clearFocus() },
            onGo = {
                focus.clearFocus()
                onIme?.invoke()
            },
        ),
        decorationBox = { inner ->
            Row(
                Modifier
                    .fillMaxWidth()
                    .clip(shape)
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    .then(if (isError) Modifier.border(1.dp, edge, shape) else Modifier)
                    .heightIn(min = 56.dp)
                    .padding(horizontal = ROW_PAD, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                RowBadge(icon)
                Box(Modifier.weight(1f)) {
                    if (value.isEmpty()) {
                        Text(placeholder, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    inner()
                }
                if (suffix != null) {
                    Text(suffix, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        },
    )
}
