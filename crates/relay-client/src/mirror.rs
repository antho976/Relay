//! The device mirror's GTK-free half: the decoder's packet gate, access units and colors (`decode`),
//! and pointer mapping, event coalescing and every `device.mirror.input` payload the client
//! sends (`input`).
pub mod decode;
pub mod input;
