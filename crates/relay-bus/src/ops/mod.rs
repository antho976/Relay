//! The op catalogue (BUS.md §10), one module per namespace. Each module declares its payload
//! and result types and its ops, and exposes `entries()`; [`all`] concatenates them in the
//! order the catalogue lists them.

/// Derive block for a payload type: strict (unknown fields are `bus.schema` errors, §5.2).
macro_rules! payload {
    ($(#[$m:meta])* $name:ident { $($body:tt)* }) => {
        #[derive(Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize, ::schemars::JsonSchema)]
        #[serde(deny_unknown_fields)]
        $(#[$m])*
        pub struct $name { $($body)* }
    };
}

/// Derive block for a result type: lenient (clients ignore unknown fields, §12).
macro_rules! result {
    ($(#[$m:meta])* $name:ident { $($body:tt)* }) => {
        #[derive(Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize, ::schemars::JsonSchema)]
        $(#[$m])*
        pub struct $name { $($body)* }
    };
}

/// `entries!(A, B, C)` → `vec![OpEntry::of::<A>(), ...]`.
macro_rules! entries {
    ($($op:ty),* $(,)?) => {
        pub fn entries() -> Vec<$crate::registry::OpEntry> {
            vec![$($crate::registry::OpEntry::of::<$op>()),*]
        }
    };
}

pub mod bus;
pub mod app;
pub mod audit;
pub mod workspace;
pub mod task;
pub mod module;
pub mod notes;
pub mod session;
pub mod overlap;
pub mod guardrail;
pub mod git;
pub mod file;
pub mod device;
pub mod provider;
pub mod notify;
pub mod ui;
pub mod money;
pub mod thread;
pub mod arbiter;
pub mod gym;
pub mod design;
pub mod picture;

/// Every op, in catalogue order.
pub fn all() -> Vec<crate::registry::OpEntry> {
    let mut v = Vec::with_capacity(200);
    v.extend(bus::entries());
    v.extend(app::entries());
    v.extend(audit::entries());
    v.extend(workspace::entries());
    v.extend(task::entries());
    v.extend(module::entries());
    v.extend(notes::entries());
    v.extend(session::entries());
    v.extend(overlap::entries());
    v.extend(guardrail::entries());
    v.extend(git::entries());
    v.extend(file::entries());
    v.extend(device::entries());
    v.extend(provider::entries());
    v.extend(notify::entries());
    v.extend(ui::entries());
    v.extend(money::entries());
    v.extend(thread::entries());
    v.extend(arbiter::entries());
    v.extend(gym::entries());
    v.extend(design::entries());
    v.extend(picture::entries());
    v
}
