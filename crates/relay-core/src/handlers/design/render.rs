//! `design.render`'s engine side (render-SPEC-v2 §5): the desktop app's answer, unlocked, and the
//! socket door's registrations for the render and its lease. Filled by render-state and
//! render-door.

use crate::engine::Engine;
use relay_bus::ops::design::RenderAnswer;

pub(super) fn register(e: &mut Engine) {
    // Unlocked (D149): the answer decodes and checks up to 4 MiB of picture and writes it beside
    // the store, none of which needs the store. The app sends it on its render connection, which
    // carries only unlocked queries, so nothing there waits behind it.
    e.register_unlocked::<RenderAnswer>(|ctx, p| Ok(crate::design_render::answer(ctx.engine(), p)));
}
