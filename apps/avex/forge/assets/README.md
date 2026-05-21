# Assets

Add the following files before building:

- `icon.png` — 1024x1024, app icon
- `splash.png` — 1242x2436 (or similar), splash screen
- `adaptive-icon.png` — 1024x1024, Android adaptive icon foreground
- `fonts/` directory with:
  - `Inter-Regular.ttf` (or Inter-Variable.ttf)
  - `Inter-Bold.ttf`
  - `BebasNeue-Regular.ttf`

Until these exist, `expo start` will work for development but the build will warn.
Get the fonts from https://fonts.google.com — search "Inter" and "Bebas Neue".

Quick icon hack: any 1024x1024 PNG works as a placeholder. Make it the FORGE diamond
in orange on the warm-brown background (#1a1410) and you're done.
