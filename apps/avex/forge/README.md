# ◆ FORGE

A personal training tracker. Built for Antho — 4-day Upper/Lower split, custom equipment (MWM-989 home gym + adjustable DBs + bench + treadmill), with logging, swap variants, analytics, trophies, and a cardio tab.

**Status:** Expo project skeleton. Web React code exists at `src/WorkoutTracker.web.jsx` (~3000 lines). Needs porting to React Native syntax before it runs on Android.

---

## Quick start

```bash
# 1. Install dependencies
npm install

# 2. Install Expo Go on your phone (App Store / Play Store)

# 3. Start the dev server
npx expo start

# 4. Scan the QR code with Expo Go (Android) or Camera (iOS)
```

The app will load on your phone with hot reload. Right now it shows a "port in progress" message. Do the port (see below), update `App.js`, save — your phone will reload automatically.

---

## Project structure

```
forge/
├── App.js                     # Entry point (currently a placeholder)
├── app.json                   # Expo config (app name, icon, package id)
├── babel.config.js            # Babel + Reanimated plugin
├── package.json               # Dependencies
├── assets/                    # icon, splash, adaptive-icon (add your own)
└── src/
    ├── WorkoutTracker.web.jsx # ORIGINAL web React code (reference)
    └── WorkoutTracker.jsx     # PORT TARGET — empty, fill this in
```

---

## Porting checklist (web React → React Native)

The web code uses HTML elements and browser APIs that don't exist in React Native. Each item below is a find-and-replace pattern you'll go through.

### Components

| Web                            | React Native                          |
|--------------------------------|---------------------------------------|
| `<div>`                        | `<View>`                              |
| `<span>` / `<p>` / `<h1>`     | `<Text>` (RN has no inline text — everything must be wrapped in `<Text>`) |
| `<button>`                     | `<Pressable>` or `<TouchableOpacity>` |
| `<input>`                      | `<TextInput>` from `react-native`     |
| `<textarea>`                   | `<TextInput multiline>`               |
| `<select>` / `<option>`        | use `@react-native-picker/picker` or build a custom picker |
| `<a href>`                     | `Linking.openURL()` from `react-native` |
| `<svg>` + children             | `react-native-svg` (`Svg`, `Circle`, `Polyline`, `Rect`, `Text` from that package) |
| `<pre>`                        | `<Text style={{ fontFamily: 'monospace' }}>` |

### Layout / CSS

- React Native uses a **subset of flexbox** by default. `display: flex` is implicit on every `View`, default direction is `column` (not `row` like web).
- **No CSS** files. All styling is JS objects via `StyleSheet.create({...})` or inline `style={{...}}`.
- **No CSS classes**, no `:hover`, no `:active` via CSS — handle interaction states in JS (`Pressable` exposes `pressed` state).
- **Units are unitless numbers**, not strings. `padding: 16` not `padding: '16px'`.
- **No `vh` / `vw` / `%` for most properties** — use `Dimensions.get('window')` for screen size.
- **No CSS keyframes** — use `react-native-reanimated` for animations (`useSharedValue`, `useAnimatedStyle`, `withTiming`, `withSpring`).
- **No `box-shadow`** — use `shadowColor`, `shadowOffset`, `shadowOpacity`, `shadowRadius` on iOS and `elevation` on Android.
- **No `backdrop-filter: blur`** — `BlurView` from `expo-blur` does this on iOS, no real equivalent on Android (skip or approximate with dark overlay).
- **No `linear-gradient` / `radial-gradient` in CSS** — use `<LinearGradient>` from `expo-linear-gradient`. Radial gradients have no native equivalent — fake with stacked LinearGradients, an SVG `radialGradient`, or just drop them.
- **No `writing-mode: vertical-rl`** — for the rotated day-spine words, use `transform: [{ rotate: '-90deg' }]` on a `<Text>`.
- **No `position: fixed`** — use `Modal` from `react-native` for overlays, or absolute-positioned `View` inside a wrapper.

### Browser APIs that need replacing

| Web                                       | React Native                                    |
|-------------------------------------------|-------------------------------------------------|
| `window.storage.get/set/delete/list`      | `AsyncStorage` from `@react-native-async-storage/async-storage` — wrap it to mimic the same interface |
| `localStorage`                            | Same as above                                   |
| `setInterval` / `setTimeout`              | Works the same                                  |
| `Date.now()`, `new Date()`                | Works the same                                  |
| `navigator.clipboard.writeText`           | `Clipboard.setStringAsync()` from `expo-clipboard` |
| `document.createElement` / DOM            | Doesn't exist — rewrite without it              |
| `<a href target="_blank">`                | `Linking.openURL('https://...')`                |
| `prompt()` / `confirm()` / `alert()`      | `Alert.alert()` from `react-native`             |
| CSS `@import url(...)`                    | `expo-font` + `useFonts` hook for Google Fonts  |
| Web fonts via Google CDN                  | Download font files into `assets/fonts/` and load via `expo-font` |
| `lucide-react` icons                      | `lucide-react-native` (already in deps) — same API |

### Storage shim

The web code calls `window.storage.get/set` everywhere. Easiest port is to write a wrapper module that uses AsyncStorage with the same API:

```js
// src/storage.js
import AsyncStorage from '@react-native-async-storage/async-storage';

export const storage = {
  async get(key) {
    const v = await AsyncStorage.getItem(key);
    return v === null ? null : { key, value: v };
  },
  async set(key, value) {
    await AsyncStorage.setItem(key, value);
    return { key, value };
  },
  async delete(key) {
    await AsyncStorage.removeItem(key);
    return { key, deleted: true };
  },
  async list(prefix = '') {
    const keys = await AsyncStorage.getAllKeys();
    return { keys: keys.filter(k => k.startsWith(prefix)), prefix };
  },
};
```

Then replace `window.storage` with `storage` everywhere in the ported code.

### Font loading

The web code uses Google Fonts via CSS `@import`. In React Native:

1. Download `Inter` (variable or weights 400/500/600/700) and `BebasNeue-Regular.ttf` from Google Fonts
2. Drop them in `assets/fonts/`
3. Load them in `App.js`:

```js
import { useFonts } from 'expo-font';

export default function App() {
  const [loaded] = useFonts({
    'Inter': require('./assets/fonts/Inter-Variable.ttf'),
    'Inter-Bold': require('./assets/fonts/Inter-Bold.ttf'),
    'BebasNeue': require('./assets/fonts/BebasNeue-Regular.ttf'),
  });
  if (!loaded) return null;
  return <WorkoutTracker />;
}
```

4. In styles, reference by family name: `fontFamily: 'BebasNeue'`.

### Things that just need quick attention

- **Modals**: replace `position: fixed` overlays with `<Modal animationType="slide" transparent={true}>` from `react-native`
- **ScrollView**: web has scrolling on `<div>` by default; in RN you need an explicit `<ScrollView>` around long content
- **KeyboardAvoidingView**: wrap input-heavy screens so the keyboard doesn't cover inputs
- **SafeAreaView**: wrap top-level views so notch and home indicator don't overlap UI
- **Status bar**: configure via `<StatusBar style="light" />` from `expo-status-bar`

---

## Suggested port order

1. **Set up the shell**: get a blank `<View>` rendering with the right background color and fonts loaded. Verify on phone.
2. **Port the styles object** (`const S = {...}`): mechanically translate hex values stay the same; remove all `px` strings → numbers; remove `cursor`, `boxSizing`, web-only properties; convert shadows to RN format.
3. **Port utility functions** (`hexToRgba`, `fmtDateShort`, `fmtMMSS`, etc.) — these are pure JS, no changes needed.
4. **Port the SVG TrophyIcon component** to `react-native-svg` (drop-in API change).
5. **Port the data shape** (`PROGRAM`, `SWAPS`, `TROPHIES`) — pure data, no changes.
6. **Port the top-level component skeleton** — state hooks, persist function (using the AsyncStorage shim).
7. **Port screen by screen**: Welcome → Overview → Gym (Train + Stats) → Cardio → Trophies → Day view → Modals.
8. **Port animations**: trophy unlock pop, rest timer ring, fade-in transitions — using Reanimated.
9. **Add Android-specific touches**: notifications when rest timer ends (`expo-notifications`), haptics on PR (`expo-haptics`).

Expect 6-10 hours of focused work for a clean port. Less if you're aggressive about scope.

---

## Building an installable APK

Once it runs in Expo Go, you can build a real APK:

```bash
npm install -g eas-cli
eas login
eas build:configure
eas build --platform android --profile preview
```

EAS Build runs on Expo's servers, gives you a download link for the `.apk` ~15-30 min later. Side-load it onto your phone or publish to Play Store ($25 one-time Google dev fee).

---

## Equipment & program reference

The training program is opinionated for:
- Adjustable bench
- Treadmill
- Adjustable dumbbells (target: 5-50lb pairs)
- Marcy MWM-989 home gym (150lb stack, ~10 plates × 15lb each, leg developer, preacher pad, high/low pulleys, press arm)
- Pull-up bar

Days: Upper A (push) → Lower A (quad) → Upper B (pull) → Lower B (ham/glute)

Every exercise has 3-5 swap variants matched to this equipment. The data is in the `SWAPS` object in `WorkoutTracker.web.jsx`.

---

## License

Personal project. Do whatever.
