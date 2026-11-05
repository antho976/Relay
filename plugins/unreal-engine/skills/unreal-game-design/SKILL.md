---
name: unreal-game-design
description: Game design for an Unreal Engine 5 project - pillars, core and meta loops, player verbs, game feel, difficulty and progression, economy and balancing in data, GDDs, scoping, prototyping, playtesting and telemetry, accessibility. Use when designing, tuning, scoping or playtesting a game or mechanic, or turning design intent into data-driven assets and Gameplay Tags.
---

# Game Design in Unreal Engine

You are helping a human make a game. Design decisions belong to them. Your job is to make those
decisions explicit, write them down, put every tunable number in data rather than code, build the
cheapest prototype that answers the open question, and measure the result. This skill covers the
design side. For implementation, see `unreal-gameplay-framework`, `unreal-cpp`,
`unreal-blueprints`, `unreal-gas` (abilities, attributes, effects), `unreal-multiplayer`,
`unreal-level-environment` (spaces), `unreal-narrative` (story, quests, dialogue),
`unreal-audio` and `unreal-materials-vfx` (feedback), `unreal-ui-umg` (HUD, menus, settings screens),
`unreal-ai` (enemy behavior) and `unreal-performance` (frame budgets).

Reference files. Read each one when its topic comes up:
- `reference/gdd-template.md`: read it before you write or restructure a GDD, a one-page design or a feature spec.
- `reference/game-feel.md`: read it before you implement or tune hit-stop, screen shake, input buffering, coyote time, force feedback or other "juice".
- `reference/balancing.md`: read it before you build progression curves, an economy, damage formulas, or the CSV/Data Table/Curve Table pipeline.

## 1. Start from intent: pillars, fantasy, verbs

Before you write code for a new game or a major feature, get or propose these, and record them in
`Docs/Design/` (or wherever the project keeps design docs):

1. **Player fantasy**: one sentence. "You are a lone salvager stripping derelict ships before they fall into the sun."
2. **Design pillars**: 3 to 5 short statements that settle arguments. Each pillar must rule something *out*.
   "Every fight is readable" rules out screen-filling particle effects on enemies.
3. **Player verbs**: what the player does, as verbs (run, jump, dash, shoot, parry, loot, craft,
   talk). Keep a primary set of 3 to 6. Every verb needs an input, feedback, a counterplay or
   cost, and a reason to use it in the core loop.
4. **Core loop** (seconds to a minute): e.g. *scout, engage, loot, reposition*.
   **Session / mid loop** (minutes): e.g. *pick a mission, complete it, extract*.
   **Meta loop** (hours): e.g. *upgrade the ship, unlock sectors, story beats*.
   Draw each loop as a cycle and name what feeds into the next loop (currencies, XP, unlocks, story flags).
5. **MDA check**: for each target *aesthetic* (challenge, fantasy, discovery, expression,
   fellowship, sensation, narrative, submission), name the *dynamics* that produce it and the
   *mechanics* (rules, data) that produce those dynamics. If an aesthetic has no mechanic behind
   it, it is a wish, not a design.

Decision rule: when a feature request arrives, check it against the pillars and loops. If it feeds
no loop and serves no pillar, flag it as a cut candidate before you build it.

## 2. Make the design data-driven, in UE terms

Designers must be able to tune the game without a C++ rebuild. Map each design concept to an asset type:

| Design concept | UE representation | Notes |
|---|---|---|
| A *kind* of thing (weapon, enemy, item, ability) | `UPrimaryDataAsset` subclass, one asset per entry | Typed, can hold references, discoverable through the Asset Manager |
| Many rows of the same shape (loot table, shop stock, dialogue lines) | `UDataTable` with a `FTableRowBase` row struct | CSV/JSON round-trip, so it diffs as text |
| Numbers that vary with level, time or distance | `UCurveTable` (+ `FCurveTableRowHandle`) or `UCurveFloat` | Sampled with `Eval(X)`; CSV round-trip |
| Identity, categories, states, story flags | Gameplay Tags (`FGameplayTag`, `FGameplayTagContainer`, `FGameplayTagQuery`) | Hierarchical (`Status.Debuff.Burning`); defined in ini or native C++ |
| Global tunables (gravity feel, global damage scale) | `UDeveloperSettings` subclass (Project Settings page) or a single "GameTuning" Data Asset | Settings go to `Config/DefaultGame.ini`, which is diffable |
| Stats and modifiers in a GAS game | Attribute Sets + Gameplay Effects with `FScalableFloat` backed by Curve Tables | See `unreal-gas` |

A minimal definition asset:

```cpp
// WeaponDefinition.h  (module deps: "Core", "CoreUObject", "Engine", "GameplayTags")
#pragma once
#include "CoreMinimal.h"
#include "Engine/DataAsset.h"
#include "Engine/CurveTable.h"
#include "GameplayTagContainer.h"
#include "WeaponDefinition.generated.h"

UCLASS(BlueprintType)
class MYGAME_API UWeaponDefinition : public UPrimaryDataAsset
{
	GENERATED_BODY()
public:
	UPROPERTY(EditDefaultsOnly, BlueprintReadOnly, Category = "Weapon")
	FText DisplayName;

	UPROPERTY(EditDefaultsOnly, BlueprintReadOnly, Category = "Weapon")
	FGameplayTagContainer WeaponTags;          // e.g. Weapon.Type.Rifle, Damage.Type.Ballistic

	UPROPERTY(EditDefaultsOnly, BlueprintReadOnly, Category = "Weapon", meta = (ClampMin = "0.01"))
	float FireInterval = 0.12f;

	UPROPERTY(EditDefaultsOnly, BlueprintReadOnly, Category = "Weapon")
	FCurveTableRowHandle DamageByLevel;        // row in CT_WeaponScaling, X = upgrade level

	UPROPERTY(EditDefaultsOnly, BlueprintReadOnly, Category = "Weapon")
	TSoftObjectPtr<UStaticMesh> Mesh;          // soft: don't load every mesh with the definition

	float GetDamage(float Level) const { return DamageByLevel.Eval(Level, TEXT("UWeaponDefinition::GetDamage")); }

	virtual FPrimaryAssetId GetPrimaryAssetId() const override
	{
		return FPrimaryAssetId(TEXT("WeaponDefinition"), GetFName());
	}
};
```

Register the type under Project Settings > Game > Asset Manager > Primary Asset Types to Scan
(type `WeaponDefinition`, base class `WeaponDefinition`, directory `/Game/Data/Weapons`). This
lands in `Config/DefaultGame.ini`, which is text, so you can edit it directly.

Gameplay Tags are defined as text too. Use `Config/DefaultGameplayTags.ini`:

```ini
[/Script/GameplayTags.GameplayTagsSettings]
ImportTagsFromConfig=True
+GameplayTagList=(Tag="Ability.Dash",DevComment="Short invulnerable dash")
+GameplayTagList=(Tag="Status.Debuff.Burning",DevComment="")
```

You can also define them natively, which gives compile-time references:

```cpp
// MyGameTags.h
#pragma once
#include "NativeGameplayTags.h"
namespace MyGameTags { UE_DECLARE_GAMEPLAY_TAG_EXTERN(Ability_Dash); }
// MyGameTags.cpp
#include "MyGameTags.h"
namespace MyGameTags { UE_DEFINE_GAMEPLAY_TAG_COMMENT(Ability_Dash, "Ability.Dash", "Short invulnerable dash"); }
```

Rules:
- Never hard-code a tunable number in C++ or a Blueprint graph. Expose it as a `UPROPERTY(EditDefaultsOnly)` on a definition asset, or as a Curve Table row.
- Use soft references (`TSoftObjectPtr`, `TSoftClassPtr`) in definitions for meshes, sounds and VFX, so that loading the catalog does not load the whole game.
- Keep one source of truth. If numbers live in a spreadsheet, the spreadsheet exports CSV into `Content/Data/Source/` (or a similar folder), and the asset is reimported from it. Do not tune in both places. The full pipeline is in `reference/balancing.md`.

## 3. Game feel

Feel means the player gets a response they can read within the first 100 ms of an input. The toolkit:
- **Responsiveness**: act on press, not release. Put anticipation into animation, not into input
  delay. Use input buffering (about 100 to 200 ms) and coyote time (about 80 to 150 ms).
- **Feedback layers on a hit**: hit-stop (40 to 120 ms), camera shake, hit flash (a material
  parameter), a hit sound with pitch variation, particles, force feedback, damage numbers, and
  a hit reaction animation. Scale every layer with the impact's weight.
- **Camera**: lag, look-ahead, FOV kick on sprint or dash. Keep it subtle, and give the player an option to reduce it.
- **Tuning**: expose every feel value (duration, amplitude, curve) in a Data Asset so it can be tuned live in PIE.

Implementation recipes (`CustomTimeDilation` hit-stop, `UCameraShakeBase` with a Perlin pattern,
`ClientStartCameraShake`, force feedback, an Enhanced Input buffer, and a coyote-time override of
`ACharacter::CanJumpInternal`) are in `reference/game-feel.md`.

## 4. Difficulty, progression, economy

- **Difficulty curve**: plan the intended tension per level or mission as a graph (a sawtooth:
  ramp up, peak at a set piece, rest, then a new baseline slightly higher). Introduce each
  mechanic in four beats: *safe introduction, simple test, combination with a known mechanic,
  twist*.
- **Progression**: separate *player skill* progression from *character power* progression. When
  power outpaces the content, the game gets trivial. When the content outpaces power, it becomes a grind.
- **Economy**: list every *source* (faucet) and *sink* of each currency, with its expected rate
  per hour. Balance on rates, not on totals.
- **Store the curves** in Curve Tables (XP per level, enemy HP per zone, price per tier), not in
  formulas scattered through the code. A formula is acceptable when it lives in exactly one
  function, and its constants live in data.

The formulas, CSV formats, the Python import and validation scripts, and a TTK/DPS sanity sheet are in `reference/balancing.md`.

Difficulty options, from cheapest to most expensive: damage-taken and damage-dealt multipliers
(one `UDeveloperSettings` value or a difficulty Data Asset), enemy aggression and timing windows,
checkpoint density, and assist toggles (aim assist, slow-mo, skip puzzle). Put each difficulty
preset in its own Data Asset (`DA_Difficulty_Story`, `_Normal`, `_Hard`), and read it through one
accessor. Never branch on a difficulty enum all over the code.

## 5. Scoping

- **Pre-production answers the risky questions first.** List them (for example "is the grapple fun?" or
  "can we stream this world on Series S?"), and prototype each one before production.
- **Vertical slice**: one short, complete section at final quality that proves the pipeline and
  the pillars. **MVP**: the smallest version of the whole loop that someone can play from start to finish.
- **Milestones**: prototype, first playable, vertical slice, alpha (feature complete), beta
  (content complete), release candidate. Give each milestone exit criteria that a playtest can check.
- **Cut list**: keep a ranked list of features with cost and value (for example `Docs/Design/CutList.md`).
  When the schedule slips, cut from the bottom. Protect the core loop and the pillars.
- When a human asks for a big feature, reply with a scoped plan: which prototype comes first, and
  what can be cut, before you write any code.

## 6. Prototyping fast in UE

1. **Greybox** the space with Modeling Mode or cubes (see `unreal-level-environment`). Use no art.
2. **Build the mechanic in Blueprint first**, on top of a template (Third Person, First Person,
   Top Down) or a Game Feature. Blueprint iteration takes seconds, and the human can tweak it.
3. **Use placeholder feedback**: `PrintString`, debug draw (`DrawDebugSphere`, `DrawDebugLine`),
   engine sounds, and a single stock particle effect.
4. **Decide**: after a playtest, keep it, change it, or kill it. Record the decision.
5. **Promote to C++** once the mechanic is kept and (a) it runs every frame or per entity at
   scale, (b) other systems need to call it, (c) it needs replication or careful math, or (d) its
   Blueprint graph is getting unreadable. The usual shape is a C++ base class holding the logic and
   the `UPROPERTY` tunables, plus a Blueprint subclass holding asset references and cosmetic events
   (`BlueprintImplementableEvent`). See `unreal-cpp` and `unreal-blueprints`.
6. Keep prototype content in `/Game/Prototypes/<Feature>/`, so it can be deleted wholesale.

With the editor open, the fastest way to try a tuning change is to edit the Data Asset property through `ue_property` or `ue_python` while PIE is stopped, and then play:

```python
import unreal
with unreal.ScopedEditorTransaction("Tune dash distance"):
    da = unreal.EditorAssetLibrary.load_asset("/Game/Data/Abilities/DA_Dash")
    da.set_editor_property("dash_distance", 650.0)   # the C++ property DashDistance
    unreal.EditorAssetLibrary.save_loaded_asset(da)
print(da.get_editor_property("dash_distance"))
```

Tell the human which assets you changed. Python property names are the snake_case form of the C++
names (`bCanDash` becomes `can_dash`). If you are unsure, run `help(type(obj))` or `obj.get_editor_property(...)` first.

## 7. Playtesting and telemetry

Protocol for each session:
1. **Goal**: one or two questions ("do players find the dash?", "is level 3 too long?").
2. **Build**: a packaged Development build, or at minimum Standalone Game, not PIE, for timing and feel.
3. **Observe silently**: do not coach the player. Note where they hesitate, die, get lost or quit.
   Ask them to think aloud.
4. **Afterwards**: three open questions. Ask "what was the most frustrating moment?", not "did you like it?"
5. **Aggregate** across 5 or more testers. One tester is an anecdote, and three testers hitting the same issue is a finding.
6. **Log** findings with severity and a proposed change, and re-test after the change.

Telemetry: log events (death with position and cause, level start and end with duration, item
purchased, option changed) as JSON lines through a small `UGameInstanceSubsystem`. The code is in
`reference/balancing.md`. Useful derived metrics are time to complete, deaths per section (plot the
positions over a top-down screenshot as a heatmap), where players quit, weapon or ability usage share,
and currency balance over time. For performance during playtests use `stat unit`, `stat fps` and
Unreal Insights (`-trace=default`).

## 8. Genre notes

- **Action / melee**: readable enemy tells (anticipation of at least 300 ms for a dodgeable
  attack), clear active and recovery frames (Anim Notify States for hit windows), hit-stop, a
  cancel window, and input buffering. Limit attackers (an attack token system, so that only N
  enemies attack at once).
- **Platformer**: coyote time, a jump buffer, variable jump height (`JumpMaxHoldTime`), higher
  gravity when falling (raise `GravityScale` while `Velocity.Z < 0`), and air control
  (`AirControl` on `UCharacterMovementComponent`). Start camera design early. Measure the gaps
  from the jump metrics (see `unreal-level-environment`, `reference/level-metrics.md`).
- **Shooter**: time-to-kill targets per weapon class, recoil patterns as curves, hitscan compared to
  projectiles, aim assist (slowdown and magnetism) for gamepad, and hit markers plus audio
  confirmation. In multiplayer, decide server authority and lag compensation early (`unreal-multiplayer`).
- **RPG**: stats and formulas in Curve Tables or GAS attributes, a loot table in a Data Table, a
  quest and dialogue state (`unreal-narrative`), and save and load from day one.
- **Puzzle**: one idea per puzzle. Introduce, then combine, then subvert. Test for solution
  uniqueness. Give a hint system with escalating hints, and never punish experimentation.
- **Survival**: resource rates (hunger or temperature decay per minute) as data. Pressure should come from
  several meters whose timings interleave, and the loop must make risk pay (better loot farther from safety).

## 9. Accessibility (design for it from the start)

- **Input remapping**: use Enhanced Input. In 5.3+ Player Mappable Keys and
  `UEnhancedInputUserSettings` support runtime rebinding. Enable "User Settings" under Project
  Settings > Engine > Enhanced Input, and give remappable Input Actions Player Mappable Key
  Settings. Also offer toggle or hold alternatives, and no mandatory button mashing.
- **Subtitles**: on by default or offered at first launch, with speaker names, adjustable size, and a
  background. See `unreal-narrative` for the subtitle pipeline. `UGameplayStatics::SetSubtitlesEnabled` controls the engine's built-in subtitles.
- **Color**: never convey information by color alone. Add shape, icon or pattern. Slate has
  color-vision-deficiency simulation and correction (`SetColorVisionDeficiencyType` on the Slate
  renderer, which Lyra's settings use). Test the HUD in simulation modes.
- **Difficulty and assists**: separate toggles (damage, timing windows, aim assist, skip) instead of one slider.
- **Motion**: options for camera shake intensity, head bob, motion blur (`r.MotionBlurQuality`
  or the Post Process setting), FOV, and screen flashes. Multiply all shake scales by a user setting.
- **UI**: scalable text (DPI scale rules or a user multiplier), readable fonts at 1080p from a couch,
  and a high-contrast mode.
- Lyra (the free Epic sample) has a working reference implementation of most of these settings.

## 10. Common pitfalls

- Tuning numbers inside Blueprint graphs or C++ constants, which leaves designers unable to find or change them.
- Designing the meta loop before the core loop is fun.
- Adding juice to hide a mechanic that isn't working. Test the mechanic with the juice turned off first.
- Hard references in definition assets that pull the entire game into memory.
- Treating the vertical slice as the MVP (or the other way round), with no exit criteria.
- Playtesting only in PIE, where frame pacing and loading differ from a packaged build.
- Hit-stop through `SetGlobalTimeDilation` in multiplayer. Time dilation is per world, so it affects everyone. Use per-actor `CustomTimeDilation`.

## Relay tools for this work

- `ue_data_table` exports Data Tables to CSV/JSON files in the repository and imports them back:
  balance numbers, loot and progression stay reviewable text.
- `ue_play` is the smallest playtest: play for a few seconds, capture screenshots, probe values
  (health, positions, timers) with Python, read the log. It does not replace a human playtest of
  feel; it catches broken rules before one.

## 11. Verify your work

- [ ] Pillars, verbs and loops are written in the design doc, and the new feature maps to them.
- [ ] Every new tunable is a `UPROPERTY` on a Data Asset, a Data Table or Curve Table row, or a Developer Setting. None is hard-coded.
- [ ] Any new Gameplay Tags are in `DefaultGameplayTags.ini` or native tag files, with no typos (`ue_log` warns about missing tags when `RequestGameplayTag` fails).
- [ ] C++ builds (`ue_build`), and `ue_log` shows no new warnings or errors.
- [ ] You saved the changed assets and listed them for the human.
- [ ] Feel values are testable in PIE, and the human knows which asset to tweak.
- [ ] The playtest goal and the metrics to collect are written down before the playtest.
- [ ] Accessibility: the new input is remappable, the new information isn't shown by color alone, and the new shake or flash respects the user's scale setting.
