---
name: unreal-ui-umg
description: Game UI in Unreal Engine 5 with UMG and CommonUI - C++ widget bases with BindWidget, layout panels, anchors and DPI, activatable widgets and input routing, MVVM, focus and gamepad navigation, HUDs, world-space widgets, menus and settings, styles, localization, animations, UI performance, accessibility. Use for any HUD, menu, widget, inventory or settings screen, gamepad UI, or UI that does not react, focus or scale.
---

# Unreal UI: UMG, CommonUI and game UI design

UMG (Unreal Motion Graphics) is the designer-facing layer over Slate. A Widget Blueprint
(`WBP_*`, a binary `.uasset`) holds the widget tree, layout and animations; logic can live in a
C++ parent class. You cannot edit the widget tree as text, and Python cannot reliably build or
edit a widget tree either. So the working split is:

1. **You write C++ base widgets** (`UUserWidget` / `UCommonActivatableWidget` subclasses) that own
   logic, data flow and input, and declare the child widgets they need with `meta=(BindWidget)`.
2. **You create the Widget Blueprint asset** with Python, reparented to your C++ class, when the
   editor is open.
3. **The human lays out the tree** in the UMG designer, following exact steps you give them:
   widget type, exact name (it must match the C++ property), panel, anchors, padding, style.
4. You compile, check the log, and verify.

Deeper material:
- `reference/umg-cpp-patterns.md` - full C++ patterns: base widget, list entries, MVVM viewmodel,
  HUD ownership, WidgetComponent, settings screen with `UGameUserSettings`, Python for Widget
  Blueprints. Read before writing any widget C++.
- `reference/commonui-setup.md` - enabling and configuring CommonUI end to end (viewport client,
  input data, controller data, Enhanced Input, root layout with stacks, activatable widget code).
  Read before any menu flow that must work with a gamepad, or any project that already has the
  CommonUI plugin enabled.

Sibling skills: `unreal-cpp` (module/build rules), `unreal-blueprints` (Blueprint assets and
Python editor scripting), `unreal-gameplay-framework` (PlayerController, input, HUD class,
GameMode), `unreal-animation` (not UMG animation - skeletal), `unreal-gas` (attribute-driven HUDs).

## First checks

1. `ue_project_info`: is `CommonUI` or `ModelViewViewModel` enabled? Does the game already have a
   root layout / HUD widget? Match what exists; do not introduce CommonUI into a project that
   deliberately uses plain UMG without asking.
2. `ue_search_assets` with `class_names: ["WidgetBlueprint"]` to find existing widgets and their
   folders (`Content/UI/...` is typical).
3. Read the `.Build.cs`. UMG work needs:

```csharp
PublicDependencyModuleNames.AddRange(new string[] { "Core", "CoreUObject", "Engine", "InputCore", "UMG" });
PrivateDependencyModuleNames.AddRange(new string[] { "Slate", "SlateCore" });
// CommonUI:    "CommonUI", "CommonInput"
// MVVM plugin: "ModelViewViewModel", "FieldNotification"
```

## Widget lifecycle (C++)

| Override | When | Use for |
|---|---|---|
| `NativeOnInitialized()` | Once per widget instance, after the tree is built | Bind button delegates, one-time setup |
| `NativePreConstruct()` | Also in the designer on every change | Visual-only preview (styles, text). No gameplay access |
| `NativeConstruct()` | Every time it is added to the viewport / a parent | Subscribe to gameplay events, refresh from state |
| `NativeDestruct()` | Every time it is removed | Unsubscribe everything bound in `NativeConstruct` |
| `NativeTick(Geometry, DeltaTime)` | Every frame while visible, if ticking is enabled | Avoid. Prefer events and timers |

Rules: bind in `NativeOnInitialized` or in `NativeConstruct` + unbind in `NativeDestruct` - never
bind in `NativeConstruct` without unbinding, or removing and re-adding the widget doubles every
callback. Always call `Super::` first.

## BindWidget contract

```cpp
UPROPERTY(meta = (BindWidget))          TObjectPtr<UTextBlock>   TitleText;   // required: WBP will not compile without a "TitleText" TextBlock
UPROPERTY(meta = (BindWidgetOptional))  TObjectPtr<UImage>       Icon;        // may be null - always null-check
UPROPERTY(Transient, meta = (BindWidgetAnim)) TObjectPtr<UWidgetAnimation> IntroAnim; // Transient is required
UPROPERTY(Transient, meta = (BindWidgetAnimOptional)) TObjectPtr<UWidgetAnimation> OutroAnim;
```

- Name and type must match: the designer widget must be named exactly `TitleText` and be a
  `UTextBlock` or subclass. A mismatch is a Widget Blueprint compile error ("A required widget
  binding ... was not found").
- The bound widgets are valid from `NativeOnInitialized` on. They are null in the constructor.
- Use the most general type you need (`UPanelWidget` rather than `UVerticalBox`) so designers can
  swap panels.
- Do not also expose them `BlueprintReadWrite`; `BlueprintReadOnly` is fine if BP needs them.

## Layout panels: pick the right one

| Panel | Use | Avoid |
|---|---|---|
| Canvas Panel | Root of full-screen HUDs/menus that need anchored corners | Nesting canvases; using it inside list rows or buttons (expensive, absolute layout) |
| Overlay | Stacking layers (background + content + badge) in the same rect | Positioning by pixel offsets |
| Horizontal / Vertical Box | Rows and columns; use slot Size = Fill with ratios for responsive splits | Deep nesting for grids |
| Grid Panel / Uniform Grid | Inventory grids, key-value settings tables | Very large dynamic grids (use Tile View) |
| Size Box | Force width/height, min/max desired size | Wrapping everything "just in case" |
| Scale Box | Scale content to fit (Scale To Fit, User Specified) - logos, fixed-art panels | Text-heavy layouts (text scales unevenly) |
| Wrap Box | Tag clouds, variable-width chips | - |
| Scroll Box | Short scrollable lists | Hundreds of children - use List View / Tile View (virtualized) |
| List View / Tile View / Tree View | Data-driven lists; entries implement `IUserObjectListEntry` | Adding widgets manually in a loop |

Prefer Overlay + Boxes over Canvas everywhere except the screen root.

## Anchors and DPI scaling

- In a Canvas Panel, set the anchor preset to the region the element belongs to (top-left for a
  minimap, bottom-center for an ability bar, stretch for backgrounds). Position and offsets are
  relative to the anchor. Wrong anchors are the #1 cause of "UI flies off screen at other
  resolutions".
- Project Settings > Engine > User Interface > DPI Scaling: default rule is **Shortest Side**
  with a curve where 1080 px maps to scale 1.0. Design at 1920x1080 and let the curve scale; do
  not author at 4K. Keep "Shortest Side" for games that ship on TV and PC.
- Preview in the designer with the screen-size dropdown (and custom sizes like 1280x720,
  3840x2160, 2560x1080 ultrawide, 1080x2400 phone portrait if mobile).
- Safe zone: wrap the root content in a **Safe Zone** widget so TV overscan and phone notches
  do not clip UI. Test with `r.DebugSafeZone.Mode 1` (or `2`) and `r.DebugSafeZone.TitleRatio 0.9`.
- Minimum text size: about 24 px at 1080p for body text read from a couch (TV); 18 px is a floor
  for PC at desk distance. Anything critical should never be smaller.

## Input modes and cursor (plain UMG)

Use the `APlayerController` API (C++) or `Set Input Mode ...` nodes (BP):

```cpp
// Open a menu: UI only
FInputModeUIOnly Mode;
Mode.SetWidgetToFocus(MenuWidget->TakeWidget());
Mode.SetLockMouseToViewportBehavior(EMouseLockMode::DoNotLock);
PC->SetInputMode(Mode);
PC->SetShowMouseCursor(true);

// Back to gameplay
PC->SetInputMode(FInputModeGameOnly());
PC->SetShowMouseCursor(false);

// HUD with clickable parts while the game still gets input
FInputModeGameAndUI GameAndUI;
GameAndUI.SetHideCursorDuringCapture(false);
PC->SetInputMode(GameAndUI);
```

- UI Only: game input (Enhanced Input on the pawn) stops. Use for full-screen menus.
- Game and UI: both receive input; UI gets first refusal. Use for inventory overlays where
  you still want camera input, or for RTS-style mouse UIs.
- With **CommonUI**, do not call `SetInputMode` yourself for menus; each activatable widget
  declares its input config and CommonUI's action router applies it (see the reference).
- Pause: `UGameplayStatics::SetGamePaused(this, true)`. UI still ticks while paused; timers and
  latent actions on paused actors do not. Widget animations play while paused.

## Focus and gamepad navigation

- A widget must be focusable to receive keyboard/gamepad focus: `SetIsFocusable(true)` on a
  `UUserWidget` (the `bIsFocusable` property; in 5.x use the setter), buttons are focusable by
  default. Give focus with `SetFocus()` / `SetUserFocus(PC)` after the widget is constructed
  (e.g. at the end of `NativeConstruct` or on activation).
- Always have something focused on every gamepad-reachable screen, and restore focus to the
  element that opened a sub-screen when it closes. Losing focus = gamepad soft-lock.
- Per-widget navigation rules live in the widget's Details > Navigation: Escape (default,
  geometric), Explicit (target a named widget), Wrap, Stop, Custom (function). Use Explicit for
  irregular layouts where geometric navigation jumps wrong.
- Viewport clicks steal focus from UI in Game and UI mode; after a mouse click on the game view,
  re-focus UI when a gamepad input arrives. CommonUI handles this automatically; plain UMG does
  not.
- Test with the Widget Reflector (Tools > Debug > Widget Reflector, or console `WidgetReflector`)
  - it shows the widget under the cursor and the focus path.

## CommonUI (when to use)

Use CommonUI when the game ships on gamepad/console, has more than a couple of stacked menus, or
needs platform-correct button prompts. It provides:

- `UCommonActivatableWidget`: a screen that is activated/deactivated, declares its input config
  and desired focus target, and can handle Back.
- `UCommonActivatableWidgetStack` / `...Queue`: containers that push/pop screens and activate
  only the top one - the standard menu stack.
- Input routing: only the active widget tree receives UI actions; `ECommonInputMode`
  (Menu / Game / All) per screen.
- `UCommonButtonBase` (style assets, selection, input action binding), `UCommonTextBlock`,
  `UCommonActionWidget` (auto platform icons), `UCommonBoundActionBar` (bottom-of-screen prompts).

Setup is several project-settings steps and data assets; follow `reference/commonui-setup.md`
exactly. The most common failure is forgetting to set the Game Viewport Client Class to
`CommonGameViewportClient`, which makes all CommonUI input silently fail.

## UMG Viewmodel (MVVM plugin)

Plugin "UMG Viewmodel" (module `ModelViewViewModel`), introduced in 5.1 as **Beta**. Check its
label in Edit > Plugins for the project's engine version before recommending it for production.
- A viewmodel is a `UMVVMViewModelBase` subclass with `FieldNotify` properties; widgets bind
  fields in the designer's View Bindings panel (one-way, two-way, conversion functions). No
  per-frame polling: bindings update when the field broadcasts.
- Good fit: data-heavy screens (inventory, stats, settings) and teams with designers who bind.
- Code pattern (setter macro, broadcast of derived fields) is in `reference/umg-cpp-patterns.md`.
- Without the plugin, get the same effect by hand: C++ widget subscribes to a multicast delegate
  on the model (e.g. attribute changed) and updates its bound widgets.

## HUD and world-space UI

- Create HUD widgets on the **owning client only**: in `APlayerController::BeginPlay` guarded by
  `IsLocalController()`, or in `AHUD::BeginPlay`. Never create widgets on a dedicated server
  (`CreateWidget` is meaningless there).
- `CreateWidget<UMyHUDWidget>(PC, HUDClass)` then `AddToViewport(ZOrder)`; for split-screen use
  `AddToPlayerScreen()`. Keep a `UPROPERTY()` pointer to it so you can remove it.
- `AHUD::DrawHUD` canvas drawing is for debug; ship UMG.
- `UWidgetComponent` for world-space UI (nameplates, 3D panels). `Screen` space draws on top at
  a projected position (cheap, always readable); `World` space renders to a texture on a quad
  (perspective, occlusion, costs a render target per component). Set Draw Size, pivot, and
  lower the redraw rate for static panels. For many nameplates prefer screen space, hide by
  distance, and pool.
- World-space interaction (VR, diegetic terminals): `UWidgetInteractionComponent` on the player.

## Menu flows

Main menu: a separate map (`L_MainMenu`) with a GameMode whose PlayerController shows the menu
and sets UI input mode. Pause: a pause screen pushed on the menu stack plus `SetGamePaused`;
unpausing on pop. Settings: a screen that reads `UGameUserSettings::GetGameUserSettings()`,
edits a pending copy of values, then `ApplySettings(false)` + `SaveSettings()` on Apply, and
reverts on Back. For display mode/resolution changes, offer a "keep these settings?" countdown
(`ConfirmVideoMode()` / `RevertVideoMode()`). Custom settings (subtitles, text size, FOV,
sensitivity) go in a `UGameUserSettings` subclass registered in `DefaultEngine.ini`:

```ini
[/Script/Engine.Engine]
GameUserSettingsClassName=/Script/MyGame.MyGameUserSettings
```

Full code in `reference/umg-cpp-patterns.md`.

## Fonts, styles, localization

- Import fonts as a **Font** asset with a composite font; add cultural sub-fonts (CJK, Arabic,
  Cyrillic) or a fallback font so localized text does not render as boxes.
- Centralize styles: CommonUI style assets (`UCommonTextStyle`, `UCommonButtonStyle`,
  `UCommonBorderStyle`) or a data asset of `FSlateFontInfo`/`FButtonStyle`. Do not restyle
  every button by hand.
- All player-visible text is `FText`, never `FString`/`FName`. In C++ use
  `#define LOCTEXT_NAMESPACE "MyHUD"` ... `LOCTEXT("AmmoLabel", "Ammo")` ... `#undef LOCTEXT_NAMESPACE`,
  or `NSLOCTEXT("MyHUD", "AmmoLabel", "Ammo")`. Format with `FText::Format(LOCTEXT("Ammo", "{Current}/{Max}"), Args)`
  and numbers with `FText::AsNumber` / `FText::AsPercent` (culture-aware). String Tables for
  shared strings. Gather/compile via Tools > Localization Dashboard.
- Leave room for 30-40% longer strings (German, French); use auto-wrap and avoid fixed-width
  text boxes. Preview with the `-culture=` command line or the editor's preview language.

## UMG animations

- Author in the designer's Animations panel; bind to C++ with `BindWidgetAnim`.
- `PlayAnimation(Anim, 0.f, 1, EUMGSequencePlayMode::Forward, 1.f)`, `PlayAnimationReverse`,
  `StopAnimation`, `IsAnimationPlaying`. Completion: `BindToAnimationFinished(Anim, Delegate)`
  or override `OnAnimationFinished_Implementation`.
- Animate render transform and opacity, not layout properties (size/padding) - render transform
  changes do not trigger relayout.
- Closing screens: play the outro, remove the widget when it finishes; CommonUI stacks have
  transition settings (type, duration) on the stack widget itself.

## Performance rules

1. **No property bindings** (the "Bind" dropdown next to a property). They poll every frame on
   the game thread. Push changes from events (delegate, viewmodel field notify, explicit
   `SetText` on change).
2. **No Tick** in widgets unless genuinely per-frame (a compass). If you must, keep it cheap.
3. Collapse hidden widgets (`ESlateVisibility::Collapsed`) instead of `Hidden`; Collapsed skips
   layout. Use `SelfHitTestInvisible` / `HitTestInvisible` on decorative containers so hit-testing
   skips them.
4. **Invalidation Box** around static or rarely-changing regions caches their draw; changes inside
   invalidate it. Mark genuinely animated children Volatile or keep them outside.
   Global invalidation: `Slate.EnableGlobalInvalidation 1` (test thoroughly; bugs show as UI
   that does not update).
5. **Retainer Box** renders children to a texture, optionally at a lower phase/frequency, and
   enables post-effect materials on UI. Costs memory; use for effects or low-rate panels.
6. Virtualize lists (List View / Tile View), pool entries, avoid rebuilding children every update.
7. Measure: `stat Slate` (via `ue_console`), Unreal Insights with the `slate` trace channel, and
   the Widget Reflector's invalidation/paint debugging options.
8. Creating widgets is expensive: create menus once and reuse (`RemoveFromParent` then re-add),
   or let CommonUI stacks cache them.

## Accessibility

- Text scale: there is no engine-wide "text size" option. Store a scale value in your
  `UGameUserSettings` subclass and apply it yourself - font sizes read from a central style that
  multiplies by the setting, or a scale applied at the root of each screen. Verify the whole UI
  at 150% before shipping.
- Colour vision: never encode meaning by colour alone - add icon, shape or text. Slate renderer
  supports colour-deficiency simulation/correction (`FSlateApplication::Get().GetRenderer()->SetColorVisionDeficiencyType(...)`;
  confirm the signature in `SlateRenderer.h` for your version) - use it to test.
- Subtitles: on by default, speaker names, background plate, size options, max ~2 lines,
  ~15 characters per second.
- Remappable controls, hold-to-toggle alternatives, no time-limited UI without an option to
  extend, readable contrast (WCAG 4.5:1 for body text as a rule of thumb).

## UI/UX principles for games

- **Hierarchy**: one primary thing per screen; the most important stat (health, ammo) is biggest
  and at a consistent screen location. Secondary info is smaller or hidden until relevant.
- **Readability at distance**: test on a TV at 3 m. High contrast outlines/shadows on HUD text over
  bright scenes. Avoid thin fonts.
- **Feedback**: every input gets a response within one frame (hover, press, focus states;
  audio click). Damage, pickups and cooldowns get a visible and audible signal.
- **Consistency**: same confirm/back buttons everywhere, same position for prompts, platform-correct icons.
- **Minimal HUD**: show what the player needs to decide now; fade the rest.
- **Never soft-lock**: every screen has a way back, gamepad focus is never lost.

## Creating Widget Blueprints with Python

Create the asset reparented to your C++ class; the human builds the tree.

```python
import unreal

parent = unreal.load_class(None, "/Script/MyGame.MyHealthBarWidget")  # /Script/<Module>.<ClassNameWithoutU>
factory = unreal.WidgetBlueprintFactory()
factory.set_editor_property("parent_class", parent)
tools = unreal.AssetToolsHelpers.get_asset_tools()
with unreal.ScopedEditorTransaction("Create WBP_HealthBar"):
    wbp = tools.create_asset("WBP_HealthBar", "/Game/UI/HUD", unreal.WidgetBlueprint, factory)
unreal.EditorAssetLibrary.save_asset(wbp.get_path_name())
print(wbp.get_path_name())
```

Then give the human steps like:
> Open `/Game/UI/HUD/WBP_HealthBar`. In the Palette search "Overlay", drag it onto the root. Drag
> a **Progress Bar** into the Overlay, rename it exactly `HealthBar`, set Horizontal/Vertical
> Alignment to Fill. Drag a **Text Block** into the Overlay, rename it exactly `HealthText`,
> center it. Compile (top-left) - it must compile without "required widget binding" errors. Save.

Afterwards compile/verify with `unreal.BlueprintEditorLibrary.compile_blueprint(wbp)` and read
`ue_log` with filter `Error|Warning`. More Python (finding widgets by parent class, checking
compile state) in `reference/umg-cpp-patterns.md`.

## Common pitfalls

- Widget created on server / for non-local controller: crashes or invisible UI in multiplayer.
- Holding a widget only in a local variable: it is still referenced by the viewport while added,
  but keep a `UPROPERTY()` pointer so you can remove or reuse it.
- Calling `SetFocus` before the widget is in the viewport: focus does nothing.
- CommonUI installed but viewport client not switched: buttons ignore gamepad, Back does nothing.
- Mixing `SetInputMode` calls with CommonUI activatable widgets: they fight; pick one.
- `FString` in UI text: not localizable, gather misses it.
- Deep Canvas nesting and Scroll Boxes with hundreds of children: layout cost explodes.
- Hard references from HUD widgets to large assets (textures for every item): the HUD pulls
  them all into memory. Use soft references (`TSoftObjectPtr<UTexture2D>`) and async load.

## Verify your work

- [ ] C++ compiles (`ue_build`, editor closed, or Live Coding) and `ue_log` shows no widget
      binding errors after the WBP compiles.
- [ ] Every `BindWidget` name exists in the WBP with a compatible type.
- [ ] Screen tested at 1280x720, 1920x1080, 3840x2160 and ultrawide in the designer preview.
- [ ] Fully navigable with gamepad only, and with mouse only; focus never lost; Back works everywhere.
- [ ] No property bindings, no unnecessary Tick; hidden widgets are Collapsed.
- [ ] All visible text is `FText`; long strings do not overflow.
- [ ] In multiplayer PIE (2 clients), each client sees only its own HUD.
- [ ] Told the human exactly which assets were created/changed and which designer steps remain.
