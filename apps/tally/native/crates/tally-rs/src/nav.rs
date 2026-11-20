//! Every page the app has, as the Kotlin app's `ui/nav/AppNav.kt` routes, and which bottom tab and
//! which section pill each belongs to.
//!
//! Space pages (Overview, Entries, Insights; Budgets, Bills, Goals; Accounts; Investments) are
//! siblings: going to one replaces the one showing. Every other page is pushed over them and Back
//! pops it.

use relay_money::model::{CategoryKind, TxType};

#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    // Spend
    Overview,
    Entries,
    Insights,
    Search,
    /// A new entry (`id` none) or an existing one, optionally starting with a type, an account or a
    /// category.
    Entry { id: Option<i64>, kind: Option<TxType>, account: Option<i64>, category: Option<i64> },
    /// One entry, read: when, which account, how much, whether it repeats; Edit from there.
    EntryView { id: i64 },
    /// The entries of one category or one account.
    Transactions { category: Option<i64>, account: Option<i64> },
    // Plan
    Budgets,
    /// A category's budget, or the overall monthly one (`category` none).
    BudgetEdit { category: Option<i64> },
    Bills,
    BillEdit { id: Option<i64> },
    Goals,
    Goal { id: i64 },
    GoalEdit { id: Option<i64> },
    // Money
    Accounts,
    AccountEdit { id: Option<i64> },
    Investments,
    /// Contribution room for a registration ("TFSA", "RRSP", "FHSA").
    RoomEdit { kind: String },
    Categories,
    CategoryEdit { id: Option<i64>, kind: CategoryKind },
    // Setup
    Settings,
    Sync,
    Appearance,
    Format,
    Backup,
    Import,
    About,
    Banks,
    Onboarding,
}

/// The bottom bar's tabs, either side of the + key.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tab {
    Home,
    Entries,
    Insights,
    Search,
}

impl Route {
    /// A tab's page: drawn under the top bar and over the tab bar, replaced rather than pushed.
    /// Everything else (Budgets, Bills, Goals, Accounts, ...) is opened from the drawer or a page
    /// and pushed, with Back.
    pub fn is_space(&self) -> bool {
        matches!(self, Route::Overview | Route::Entries | Route::Insights | Route::Search)
    }

    pub fn tab(&self) -> Tab {
        match self {
            Route::Entries | Route::Transactions { .. } | Route::EntryView { .. } => Tab::Entries,
            Route::Insights => Tab::Insights,
            Route::Search => Tab::Search,
            _ => Tab::Home,
        }
    }

    /// A page the drawer opens: it shows the drawer's key instead of Back, and a swipe right
    /// brings the drawer out over it.
    pub fn is_top_level(&self) -> bool {
        matches!(
            self,
            Route::Budgets | Route::Bills | Route::Goals | Route::Accounts | Route::Investments | Route::Categories | Route::Settings | Route::Banks | Route::Import
        )
    }

    /// The top bar's title on a tab's page.
    pub fn title(&self) -> &'static str {
        match self {
            Route::Overview => "Overview",
            Route::Entries => "Entries",
            Route::Insights => "Insights",
            Route::Search => "Search",
            _ => "",
        }
    }

    /// Section pills over a page: none on this look (the drawer and the tabs replace them).
    pub fn sections(&self) -> &'static [(&'static str, Route)] {
        &[]
    }
}

impl Tab {
    pub fn home(self) -> Route {
        match self {
            Tab::Home => Route::Overview,
            Tab::Entries => Route::Entries,
            Tab::Insights => Route::Insights,
            Tab::Search => Route::Search,
        }
    }
}

/// The side menu: every place, in the order the Kotlin app's sidebar lists them.
pub const MENU: &[(&str, &str, Route)] = &[
    ("Overview", "home", Route::Overview),
    ("Entries", "list", Route::Entries),
    ("Insights", "graph", Route::Insights),
    ("Budgets", "pie", Route::Budgets),
    ("Bills", "repeat", Route::Bills),
    ("Goals", "target", Route::Goals),
    ("Accounts", "wallet", Route::Accounts),
    ("Investments", "trend", Route::Investments),
    ("Categories", "tag", Route::Categories),
    ("Connected banks", "bank", Route::Banks),
    ("Import", "download", Route::Import),
    ("Settings", "settings", Route::Settings),
];
