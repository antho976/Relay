These fonts use the exact Latin font data from Relay-2 at
`1745dd3f68b7bc786f48142ad7da591537aeb057`, distributed by its installed
`@fontsource/fira-sans`, `@fontsource/fira-sans-condensed`, and
`@fontsource/fira-mono` packages. The WOFF containers were decompressed
losslessly to native TrueType using FontTools (TTFont, flavor=None);
no outlines, metrics or weights were edited. Each family's
SIL Open Font License is included. Pango registers them only for Relay.

The one naming change: fontsource names each weight as its own family
(name ID 1 `Fira Sans Medium`, `Fira Mono Medium`, ...), so fontconfig never
offered them for `font-family: 'Fira Sans'; font-weight: 500`. The 500 and 600
files carry the typographic family and subfamily the upstream Fira TTFs have
(name ID 16 `Fira Sans` / `Fira Sans Condensed` / `Fira Mono`, name ID 17
`Medium` or `SemiBold`), added with FontTools `name.setName` on the same
platform records as name ID 1. The 400 and Condensed 700 files already use the
base family and are unchanged.

`sora-latin-600-normal.ttf` is Sora SemiBold (Latin), the Relay wordmark's face, from the
fontsource CDN build of [Sora](https://github.com/sora-xor/sora-font) under the SIL Open Font
License (`sora-LICENSE`). Its family is already `Sora`, so it is unchanged. Only the start screen
(`start.rs`) uses it.

`geist-*` are Geist (400, 500, 600, 700) and Geist Mono (400, 500), Latin, from the fontsource CDN build
of [Geist](https://github.com/vercel/geist-font) under the SIL Open Font License
(`geist-LICENSE`). Each already names its base family (`Geist`, `Geist Mono`) and is unchanged.
Geist is the interface's face and Geist Mono its figures, keycaps and code; the terminals keep
Fira Mono.
