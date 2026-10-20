//! `design.picture.*` (pictures-SPEC-v2 §5): stock photo search, thumbnails, adding one to a
//! design with its credit, and the Pexels key. The key's ops are here; search, thumb and add come
//! with pictures-search and pictures-add, and answer bus.not_implemented until then.
//!
//! Both key ops are staged: the check with Pexels and the keyring (`secret-tool`) run in prepare,
//! with nothing locked, and the finish writes nothing. Neither emits an event: only the picker
//! that sent one shows the key's state.

use crate::engine::Engine;
use crate::pictures::key;
use relay_bus::ops::picture::{KeyOut, KeyRemove, KeySet};

pub fn register(e: &mut Engine) {
    e.register_staged::<KeySet, ()>(|ctx, p| key::set(ctx.engine(), &p.key), |_, _, ()| Ok(KeyOut { saved: true }));
    e.register_staged::<KeyRemove, ()>(|ctx, _| key::remove(ctx.engine()), |_, _, ()| Ok(KeyOut { saved: false }));
}
