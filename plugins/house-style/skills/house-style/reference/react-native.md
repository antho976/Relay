# React Native and Expo

What changes when the motion in `motion.md` runs in React Native. The gate, values, gestures and
haptics there all apply. This file covers only the runtime, tools and traps. Three things drive it:
there is no hover, there are two runtimes (React Native's, where React renders, and the UI runtime,
where worklets run every frame), and the finger is on the element, so interruption and velocity
handoff are the baseline.

## Tools, cheapest first

| Need | Tool |
| --- | --- |
| A state change with no gesture (press, toggle, colour) | Reanimated CSS transition (`transitionProperty` in the style) |
| A loop, multi-stage, or play on mount | Reanimated CSS animation (`animationName` keyframes) |
| Mount, unmount, list reflow | Layout animations: `entering`, `exiting`, `itemLayoutAnimation` |
| Anything a finger touches, or derived from scroll | `useSharedValue` + `Gesture` + `useAnimatedStyle` |
| Screen to screen | Expo Router native stack options, never hand-built |
| A sheet that is its own screen | `presentation: 'formSheet'` |
| Tab bar | `NativeTabs` (`expo-router/unstable-native-tabs`); tab switches use `animation: 'none'` |
| Context menu, press-and-hold preview (iOS) | `Link.Menu` / `Link.Preview` |
| Header collapsing into a large title (iOS) | `headerLargeTitleEnabled` |
| Pull to refresh | `RefreshControl` |
| UI that tracks the keyboard | `react-native-keyboard-controller` (with `KeyboardProvider` at the root) |
| Illustration, celebration | Lottie, never for UI state |
| A huge animated scene, free drawing | `@shopify/react-native-skia` |

Use a shared value only when the value is continuous or interruptible. A press scale is a CSS
transition. Install with `npx expo install <package>` so versions match the SDK:
`react-native-reanimated` + `react-native-worklets`, `react-native-gesture-handler`, `expo-router`,
`expo-haptics`.

## Keep motion on the UI runtime

- **Reanimated, not core `Animated`,** and `Gesture.Pan()`, not `PanResponder`.
- **Never `setState` from a gesture or scroll handler.** Shared value into `useAnimatedStyle`, so React
  never re-renders.
- **`scheduleOnRN(fn, ...args)`** from `react-native-worklets` replaces the deprecated `runOnJS`.
  Call it in `onEnd` or a `useAnimatedReaction` at a threshold, never in `onUpdate` (60–120 times a
  second).
- **Never read or write a shared value during render.** Use `.get()` / `.set()` (compiler-safe,
  `set` takes an updater) in worklets, handlers and effects only.
- **Functions called from a worklet start with `'worklet'`,** or they throw on device while working
  in the debugger.
- **Wrap gestures in `useMemo`** on Gesture Handler v2. On v3 each gesture is a hook (`onStart` →
  `onActivate`, `onEnd` → `onDeactivate`).

## Properties

- `width`, `height`, `margin`, `padding`, `flex`, `top`, `left` and `gap` re-run layout for the node
  and its siblings. The exception is the absolute, childless element from `motion.md` §4.
- `transform` is an array and order matters: `[{ translateY }, { scale }]` scales after moving.
- Don't animate Android `elevation` or `BlurView` intensity. Crossfade a static pre-shadowed or
  blurred layer instead.
- `translateY('100%')` moves a sheet by its own height.

## Press, not hover

Every hover affordance is redesigned, not ported. Feedback on press-in and commit on press-out, with
`scale: 0.97` over 100–150 ms on `Pressable`. Targets are 44×44 pt (48 dp), grown with `hitSlop`
rather than a bigger visual, and `pressRetentionOffset` stops a drifting finger from cancelling a
press. Android ripple only in a Material-styled app.

Haptics from `expo-haptics`: `selectionAsync()` for a detent or picker tick, `impactAsync(Light)` for
a snap or commit, `impactAsync(Medium)` for a heavy landing or destructive act,
`notificationAsync(Success | Error)` for outcomes. From a worklet:
`scheduleOnRN(Haptics.selectionAsync)`.

Reduced motion: `useReducedMotion()`, or `reduceMotion: ReduceMotion.System` per animation; screen
transitions become `animation: 'fade'`. `allowFontScaling` is on, so measure with `onLayout`.

## Setup that silently breaks motion

- Reanimated 4 needs the New Architecture. `babel-preset-expo` adds the worklets plugin; in a bare
  project it goes last, and a missing plugin throws `Failed to create a worklet`.
- `GestureHandlerRootView` must wrap the app (the root `_layout`), or gestures do nothing without an
  error.
- ProMotion iPhones cap third-party animation at 60 fps unless `ios.infoPlist`
  `CADisableMinimumFrameDurationOnPhone` is `true`. Recent SDKs set it. Then the budget is 8 ms.
- Expo Go and dev builds are not where you judge feel: use a release build on the slowest device you
  support.
- `entering` on a virtualised list row misbehaves: animate the container, or use
  `itemLayoutAnimation`.
