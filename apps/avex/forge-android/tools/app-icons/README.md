# App icon generator

Every launcher icon is the Avex mark in its own colour, material and backdrop. The art is generated
from SVG, so the mark is identical across the whole set and a change to it is one re-run.

- `emblem.mjs` holds the mark's geometry (the 108-unit adaptive grid).
- `icons.mjs` holds the set: one entry per `AppIcon` enum constant, in picker order, each a
  backdrop layer (`bg`) and a mark layer (`fg`).
- `generate.mjs` renders them.

```
node generate.mjs --preview /some/dir   # contact sheet (squircle, circle, 48px) only
node generate.mjs                        # write the Android resources
```

Needs `rsvg-convert` (librsvg) and ImageMagick's `magick`.

## What it writes

| File | Use |
|---|---|
| `res/drawable-nodpi/app_icon_<name>_bg.webp` | adaptive background layer, 432 px (icons whose layer is identical share the first one's file) |
| `res/drawable-nodpi/app_icon_<name>_fg.webp` | adaptive foreground layer, 432 px, transparent |
| `res/drawable-nodpi/app_icon_<name>.webp` | the flattened visible square, for the picker |
| `res/drawable/ic_launcher_monochrome.xml` | the mark as a vector, the themed-icon layer of every icon |
| `res/mipmap-anydpi-v26/ic_launcher[_<name>].xml` | the adaptive icons |
| `play-store-icon-512.png` (repo root) | the store listing icon |

## Adding or cutting an icon

1. Add or remove the entry in `icons.mjs` and the matching constant in `AppIcon.kt` (same name,
   same order), with a `launchPalette` for the launch wordmark.
2. Add or remove its `.icon.<Name>` activity-alias in the manifest.
3. Re-run `node generate.mjs`.

**Cutting an icon that has shipped:** do not delete its alias. Add its name to `AppIcon.RETIRED`
pointing at the nearest survivor, and keep the alias disabled with the survivor's mipmap. A device
whose enabled alias disappears from the manifest loses its launcher entry entirely.
