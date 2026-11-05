---
name: unreal-gas
description: The Gameplay Ability System in Unreal Engine 5 C++ - AbilitySystemComponent placement and replication, AttributeSets, GameplayEffects, abilities and AbilityTasks, Gameplay Tags and Cues, input binding, prediction and init-order bugs. Use when adding or debugging abilities, attributes, damage, buffs, cooldowns or anything on UAbilitySystemComponent.
---

# Gameplay Ability System (GAS)

GAS is a framework for actions (abilities), numeric stats (attributes), timed/instant
modifications (effects), state flags (tags) and cosmetic feedback (cues), with built-in
replication and client prediction. It is powerful and heavy. Read this whole file before
touching GAS code. Reference code:
- `reference/gas-setup-cpp.md` - full minimal setup (Build.cs, native tags, AttributeSet,
  PlayerState-owned ASC, Character init and Enhanced Input binding, editor assets to create).
  Read it whenever you add GAS to a project or a new character type.
- `reference/gas-abilities-effects-cpp.md` - a montage-driven ability with AbilityTasks, a
  damage execution calculation and a cost MMC. Read it before writing abilities or calculations.

Related: `unreal-multiplayer` (replication fundamentals), `unreal-cpp` (module/build
basics), `unreal-gameplay-framework` (PlayerState, Controller, Pawn lifecycles), `unreal-animation`
(montages and notifies used by abilities).

## When to use GAS, and when not

Use it when:
- The game has many abilities/skills/spells with costs, cooldowns, stacking buffs/debuffs,
  status effects, or RPG-like stats.
- The game is networked and abilities need client-side prediction (feels instant on the
  client, corrected by the server).
- Designers need to author abilities and effects as data (Blueprint subclasses, GE assets).

Avoid it when:
- A single-player game with a handful of actions and a health float. A plain
  `UActorComponent` with a few functions is faster to write and debug.
- The team cannot invest in learning it. GAS failures are silent (an ability just does not
  activate) and debugging requires knowing its internals.
- You only need tags: `GameplayTags` works standalone without GAS.

## Module and plugin setup

1. Enable the plugin in the `.uproject` (`Plugins` array): `{ "Name": "GameplayAbilities", "Enabled": true }`.
   GameplayTags and GameplayTasks are engine modules and need no plugin entry.
2. `<Module>.Build.cs`:
   ```csharp
   PublicDependencyModuleNames.AddRange(new string[] {
       "Core", "CoreUObject", "Engine", "InputCore", "EnhancedInput",
       "GameplayAbilities", "GameplayTags", "GameplayTasks" });
   ```
3. Global data: `UAbilitySystemGlobals::Get().InitGlobalData()` must run once before GAS is
   used (it loads tag/cue/global tables and target data script structs). In UE 5.3+ the
   engine calls it automatically. On 5.0-5.2 call it from a custom
   `UAssetManager::StartInitialLoading()` (and set `AssetManagerClassName` in
   `DefaultEngine.ini`). If unsure, grep the engine for `InitGlobalData` callers.
4. Build with `ue_build` (editor closed) and check `ue_log` with filter `Error|Warning`.

## Where the AbilitySystemComponent (ASC) lives

| Placement | Use for | Consequence |
|---|---|---|
| On the Pawn/Character | AI, enemies, simple games, pawns that do not respawn with persistent state | Attributes/effects are destroyed with the pawn |
| On the PlayerState | Player characters that respawn, or swap pawns | State persists across death; PlayerState must replicate often |

Rules:
- Whatever actor owns the ASC is the **OwnerActor**; the pawn in the world is the
  **AvatarActor**. For Pawn placement both are the pawn.
- Implement `IAbilitySystemInterface` (`GetAbilitySystemComponent()`) on the owner actor
  **and** on the pawn (the pawn returns the PlayerState's ASC). Many GAS helpers use this.
- PlayerState default replication rate is too low for GAS. In its constructor set
  `SetNetUpdateFrequency(100.f)` (5.5+; before 5.5 assign `NetUpdateFrequency = 100.f`).
- `AbilitySystemComponent->SetIsReplicated(true)` always.

### Replication mode (`ASC->SetReplicationMode(EGameplayEffectReplicationMode::X)`)

| Mode | GEs replicated to | Tags/Cues | Use for |
|---|---|---|---|
| `Full` | Every client | Every client | Single-player, small co-op; costly at scale |
| `Mixed` | Owning client only | Everyone | Player-controlled characters in multiplayer |
| `Minimal` | Nobody | Everyone | AI / non-player actors |

`Mixed` requires the OwnerActor's `Owner` to be the Controller. PlayerState is owned by its
controller automatically; a Pawn is owned by its controller after `PossessedBy`. If you
spawn an ASC-owning actor yourself, `SetOwner(Controller)`.

## Init order (the number one source of GAS bugs)

`ASC->InitAbilityActorInfo(OwnerActor, AvatarActor)` must be called **on the server and on
the owning client** after both actors exist. Symptoms of getting it wrong: abilities never
activate on the client, `ActorInfo` is null, montages do not play, attributes read zero.

ASC on PlayerState:
- Server: `ACharacter::PossessedBy(AController*)` - PlayerState is valid there.
- Owning client: `APawn::OnRep_PlayerState()` - the first point the client knows its PlayerState.
- Also call it again whenever the avatar changes (new pawn after respawn).

ASC on the Character:
- Server: `PossessedBy` (after `Super`).
- Client: `APawn::OnRep_Controller()` (or `AcknowledgePossession` on the PlayerController).
- AI: `PossessedBy` covers it; AI has no owning client.

Only the server grants abilities (`GiveAbility`) and applies startup effects. Guard with
`HasAuthority()`, and skip abilities already granted (`ASC->FindAbilitySpecFromClass(Class)`)
so re-possession or respawn does not grant them twice.

## AttributeSets

- Subclass `UAttributeSet`. Each attribute is a `UPROPERTY` of type
  `FGameplayAttributeData`. Use the accessor macro (define it yourself; the engine only
  documents it):
  ```cpp
  #define ATTRIBUTE_ACCESSORS(ClassName, PropertyName) \
      GAMEPLAYATTRIBUTE_PROPERTY_GETTER(ClassName, PropertyName) \
      GAMEPLAYATTRIBUTE_VALUE_GETTER(PropertyName) \
      GAMEPLAYATTRIBUTE_VALUE_SETTER(PropertyName) \
      GAMEPLAYATTRIBUTE_VALUE_INITTER(PropertyName)
  ```
  This gives `GetHealthAttribute()` (static), `GetHealth()`, `SetHealth()`, `InitHealth()`.
- Create the set as a default subobject of the **OwnerActor** (the ASC's owner) in its
  constructor; the ASC discovers it. For sets added at runtime use `ASC->AddAttributeSetSubobject(NewSet)`.
- Each attribute has a **BaseValue** (permanent; changed by Instant and Periodic GEs) and a
  **CurrentValue** (Base + active Duration/Infinite modifiers).
- Clamping:
  - `PreAttributeChange(const FGameplayAttribute&, float& NewValue)` - clamps the
    *CurrentValue* before it changes. It does not change the base and fires for aggregate
    recalculation; do not put gameplay logic (death, events) here.
  - `PreAttributeBaseChange(const FGameplayAttribute&, float& NewValue) const` - clamp base.
  - `PostGameplayEffectExecute(const FGameplayEffectModCallbackData& Data)` - runs on the
    server after an **Instant/Periodic** GE changed a base value. Put damage-to-health
    conversion, clamping of the result, and death detection here. Include
    `GameplayEffectExtension.h` for `FGameplayEffectModCallbackData`.
- Use a **meta attribute** (`IncomingDamage`, not replicated) that damage GEs write to; in
  `PostGameplayEffectExecute` subtract it from Health after armor/shields, then zero it.
- Max values (MaxHealth) are separate attributes; when Max changes, rescale or clamp current
  in `PostAttributeChange` / `PreAttributeChange`.
- Replication:
  ```cpp
  UPROPERTY(BlueprintReadOnly, ReplicatedUsing = OnRep_Health, Category = "Attributes")
  FGameplayAttributeData Health;
  UFUNCTION() void OnRep_Health(const FGameplayAttributeData& OldHealth);

  void UMyAttributeSet::OnRep_Health(const FGameplayAttributeData& OldHealth)
  { GAMEPLAYATTRIBUTE_REPNOTIFY(UMyAttributeSet, Health, OldHealth); }

  void UMyAttributeSet::GetLifetimeReplicatedProps(TArray<FLifetimeProperty>& Out) const
  {
      Super::GetLifetimeReplicatedProps(Out);
      DOREPLIFETIME_CONDITION_NOTIFY(UMyAttributeSet, Health, COND_None, REPNOTIFY_Always);
  }
  ```
  `REPNOTIFY_Always` is required so predicted values get reconciled even if the server
  value equals the old client value. `GAMEPLAYATTRIBUTE_REPNOTIFY` is what informs the ASC.
- UI: bind to `ASC->GetGameplayAttributeValueChangeDelegate(UMyAttributeSet::GetHealthAttribute()).AddUObject(...)`.
  Do not poll in Tick.
- Initialize attributes with an Instant GE (`GE_DefaultAttributes`, Override modifiers) or a
  DataTable (`FAttributeMetaData`), not by calling setters from random places.

## GameplayEffects (GE)

GEs are data-only: create Blueprint subclasses of `UGameplayEffect` in the editor, or C++
subclasses that fill properties in the constructor. Never subclass for logic.

Duration policy (`DurationPolicy`):
- `Instant` - permanently changes BaseValue (damage, heal, cost). Cannot grant tags.
- `HasDuration` - temporary modifiers on CurrentValue, removed on expiry (buffs, cooldowns).
- `Infinite` - until removed explicitly (`RemoveActiveGameplayEffect(Handle)`), e.g. auras, equipment.
- `Period` > 0 turns Duration/Infinite GEs into periodic ticks that act like Instant
  executions each period (DoT, regen).

Modifiers (`FGameplayModifierInfo`): target attribute, `ModifierOp` (`Additive`,
`Multiplicitive` - the engine spells it this way, `Division`, `Override`; newer engines add
more, see `EGameplayModOp` in `GameplayEffectTypes.h`) and a magnitude:
- `Scalable Float` (optionally scaled by a CurveTable row and GE level),
- `Attribute Based` (from a source/target attribute, snapshot or live),
- `Custom Calculation Class` (MMC, below),
- `Set By Caller` (value passed at runtime keyed by a tag: `Spec->SetSetByCallerMagnitude(Tag, Value)`).

GE components (5.3+): tags granted to the target, application requirements, immunity,
removal of other effects, granting abilities, etc. moved from flat GE properties to the
**Components** array (`UTargetTagsGameplayEffectComponent`, `UAssetTagsGameplayEffectComponent`,
`UAbilitiesGameplayEffectComponent`, ...). Older tutorials showing `InheritableOwnedTagsContainer`
are pre-5.3 and produce deprecation warnings.

Stacking: `StackingType` (None, AggregateBySource, AggregateByTarget), `StackLimitCount`,
duration refresh and period reset policies. Decide stacking before designers author GEs.

**MMC** (`UGameplayModMagnitudeCalculation`): computes one modifier's magnitude from captured
attributes and tags. Works with any duration policy and can be predicted. Use for "cost = 10%
of max mana", "cooldown scales with haste".

**Execution calculation** (`UGameplayEffectExecutionCalculation`): can read many captured
attributes and output many modifiers. Only in Instant or Periodic GEs, **server-only, never
predicted**. Use for damage formulas (attack vs. armor, crits). Declare captures with
`DECLARE_ATTRIBUTE_CAPTUREDEF` / `DEFINE_ATTRIBUTE_CAPTUREDEF` and add them to
`RelevantAttributesToCapture` in the constructor. Examples in `reference/gas-abilities-effects-cpp.md`.

Applying from C++:
```cpp
FGameplayEffectContextHandle Ctx = SourceASC->MakeEffectContext();
Ctx.AddSourceObject(this);
FGameplayEffectSpecHandle Spec = SourceASC->MakeOutgoingSpec(DamageEffectClass, Level, Ctx);
if (Spec.IsValid())
{
    Spec.Data->SetSetByCallerMagnitude(TAG_Data_Damage, 25.f);
    SourceASC->ApplyGameplayEffectSpecToTarget(*Spec.Data.Get(), TargetASC);
}
```
Inside an ability prefer the ability helpers (`MakeOutgoingGameplayEffectSpec`,
`ApplyGameplayEffectSpecToOwner`, `ApplyGameplayEffectSpecToTarget`) so the prediction key
is carried. Get a target ASC with
`UAbilitySystemBlueprintLibrary::GetAbilitySystemComponent(Actor)`.

## GameplayAbilities (GA)

Subclass `UGameplayAbility`. Configure in the constructor:
```cpp
InstancingPolicy   = EGameplayAbilityInstancingPolicy::InstancedPerActor;
NetExecutionPolicy = EGameplayAbilityNetExecutionPolicy::LocalPredicted;
```
Instancing:
- `InstancedPerActor` - one instance reused; can keep state and run AbilityTasks. Default choice.
- `InstancedPerExecution` - new object per activation; simplest mental model, more garbage.
- `NonInstanced` - runs on the CDO, no state, no tasks. Deprecated in 5.5; do not use in new code.

Net execution:
- `LocalPredicted` - activates on the client immediately, server confirms or rolls back. Player actions.
- `LocalOnly` - client only (UI, local cosmetics).
- `ServerInitiated` - server activates, then the owning client.
- `ServerOnly` - server only (AI, passive triggers).

Tags on the ability (set in constructor or Blueprint defaults):
- Asset tags identify the ability (`AbilityTags`; in 5.5+ use `SetAssetTags()` in the
  constructor - the old member is deprecated).
- `ActivationOwnedTags` - granted to the owner while active.
- `ActivationRequiredTags` / `ActivationBlockedTags` - gate activation on owner tags (e.g. block with `State.Dead`, `State.Stunned`).
- `CancelAbilitiesWithTag` / `BlockAbilitiesWithTag` - affect other abilities.

Lifecycle: `CanActivateAbility` -> `ActivateAbility` -> `CommitAbility` -> ... -> `EndAbility`.
- `CommitAbility(Handle, ActorInfo, ActivationInfo)` checks and applies the **cost** GE
  (`CostGameplayEffectClass`, Instant, negative modifier) and **cooldown** GE
  (`CooldownGameplayEffectClass`, HasDuration, grants a `Cooldown.*` tag via the target tags
  component). If it returns false, call `EndAbility(..., true, true)` and return.
- **Always end the ability** on every path (success, cancel, task interrupted). A
  never-ended InstancedPerActor ability blocks re-activation forever.

Granting and activation:
```cpp
// Server only
FGameplayAbilitySpecHandle H = ASC->GiveAbility(FGameplayAbilitySpec(AbilityClass, 1, InputID, this));
ASC->TryActivateAbilityByClass(AbilityClass);
ASC->TryActivateAbilitiesByTag(FGameplayTagContainer(TAG_Ability_Dash));
UAbilitySystemBlueprintLibrary::SendGameplayEventToActor(Actor, TAG_Event_Hit, Payload); // triggers abilities with an event trigger
```

AbilityTasks (async steps inside an instanced ability): create with the task's static
factory, bind delegates, then call `ReadyForActivation()`. Common engine tasks:
`UAbilityTask_PlayMontageAndWait`, `UAbilityTask_WaitGameplayEvent`,
`UAbilityTask_WaitDelay`, `UAbilityTask_WaitInputRelease`, `UAbilityTask_WaitTargetData`,
`UAbilityTask_WaitGameplayTagAdded`/`Removed`, `UAbilityTask_WaitAttributeChange`.
Play montages through `PlayMontageAndWait`, not `AnimInstance->Montage_Play`, so GAS can
replicate and predict them. Example in `reference/gas-abilities-effects-cpp.md`.

## Gameplay Tags

- Hierarchical names (`State.Dead`, `Ability.Movement.Dash`). Matching `A.B` against `A`
  succeeds with `HasTag`/`MatchesTag`; use `HasTagExact`/`MatchesTagExact` to avoid that.
- Prefer native tags in C++ so code never uses string literals:
  ```cpp
  // MyTags.h
  #include "NativeGameplayTags.h"
  UE_DECLARE_GAMEPLAY_TAG_EXTERN(TAG_State_Dead);
  // MyTags.cpp
  UE_DEFINE_GAMEPLAY_TAG_COMMENT(TAG_State_Dead, "State.Dead", "Owner is dead");
  ```
  `UE_DEFINE_GAMEPLAY_TAG_STATIC` defines a tag private to one .cpp.
- Designer-only tags go in `Config/DefaultGameplayTags.ini`:
  `[/Script/GameplayTags.GameplayTagsSettings]` then `+GameplayTagList=(Tag="Status.Burning",DevComment="")`.
- Query owner tags: `ASC->HasMatchingGameplayTag(TAG)`; react:
  `ASC->RegisterGameplayTagEvent(TAG, EGameplayTagEventType::NewOrRemoved).AddUObject(...)`.
- Loose tags (`AddLooseGameplayTag`) are **not replicated**; use GEs to grant replicated
  tags, or `AddReplicatedLooseGameplayTag` (verify its availability in your version's ASC header).

## Gameplay Cues (cosmetics)

- Tag must start with `GameplayCue.` (e.g. `GameplayCue.Hit.Fire`).
- `UGameplayCueNotify_Static` (or `UGameplayCueNotify_Burst`) for one-shots from Instant
  GEs/`ExecuteGameplayCue`; `AGameplayCueNotify_Actor` (or `AGameplayCueNotify_Looping`)
  for looping effects tied to a Duration/Infinite GE (OnActive/WhileActive/OnRemove).
- Trigger from GEs (Gameplay Cues section of the GE) or manually:
  `ASC->ExecuteGameplayCue(Tag, Params)`, `AddGameplayCue`, `RemoveGameplayCue`.
- Cues are cosmetic, unreliable and may not play on every client. Never put gameplay logic in cues.
- Restrict the scan to a folder for faster startup, `DefaultGame.ini`:
  `[/Script/GameplayAbilities.AbilitySystemGlobals]` `+GameplayCueNotifyPaths="/Game/GAS/Cues"`.
  Cue notify assets outside those paths will not be found.
- Visual/audio content for cues: see `unreal-materials-vfx` (Niagara) and `unreal-audio`.

## Input with Enhanced Input

GAS predates Enhanced Input. Do not use the legacy `BindAbilityActivationToInputComponent`.
Pattern (full code in `reference/gas-setup-cpp.md`):
1. Define an `enum class EAbilityInputID : uint8 { None, Confirm, Cancel, Ability1, ... }`.
2. Grant each ability with `FGameplayAbilitySpec(Class, Level, static_cast<int32>(InputID))`.
3. In `SetupPlayerInputComponent`, bind each `UInputAction` twice with a payload:
   `EIC->BindAction(Action, ETriggerEvent::Started, this, &AMyChar::AbilityInputPressed, int32(ID));`
   and `ETriggerEvent::Completed` to `AbilityInputReleased`.
4. Handlers call `ASC->AbilityLocalInputPressed(ID)` / `AbilityLocalInputReleased(ID)`. These
   activate the matching spec if not active, or forward input to the active ability
   (`WaitInputRelease` depends on this).
Use `Started`, not `Triggered` - `Triggered` fires every frame while held. Tag-based input
(Lyra style, input tag stored in the spec's dynamic source tags) scales better for large
projects; it needs an ASC subclass because `AbilitySpecInputPressed` is protected.

## Prediction basics

- A `LocalPredicted` ability creates an `FPredictionKey`; effects applied in it on the client
  are predicted and later reconciled when the server's replicated state arrives.
- Predicted: ability activation, GE application (not removal), attribute changes from
  modifiers, owned tags from GEs, cues, montages (via the task), cooldowns.
- Not predicted: GE removal, periodic effect ticks, execution calculations, spawning actors
  (spawn projectiles on the server; fake a cosmetic one on the client if needed), damage
  dealt to others.
- Client-side hit detection: send target data (`UAbilityTask_WaitTargetData` or
  `ServerSetReplicatedTargetData`) and validate on the server.
- Anything with authority-only consequences (applying damage, killing) belongs behind
  `HasAuthority(&CurrentActivationInfo)` inside the ability, or in server-only code.

## Workflow: add a new ability

1. Native tag for the ability (`Ability.X`) and any cooldown tag (`Cooldown.X`).
2. C++ `UGameplayAbility` subclass (or a Blueprint subclass of an existing base).
3. In the editor create cost and cooldown GE Blueprints. Through MCP:
   ```python
   import unreal
   at = unreal.AssetToolsHelpers.get_asset_tools()
   f = unreal.BlueprintFactory()
   f.set_editor_property("parent_class", unreal.GameplayEffect)
   with unreal.ScopedEditorTransaction("Create GE_Cooldown_Dash"):
       ge = at.create_asset("GE_Cooldown_Dash", "/Game/GAS/Effects", None, f)
       unreal.EditorAssetLibrary.save_asset(ge.get_path_name())
   print(ge.get_path_name())
   ```
   Setting GE CDO properties (duration, components) from Python is fiddly because the
   structs are nested instanced objects; give the human click-paths instead: open the GE,
   Details > Duration Policy = Has Duration, Scalable Float Magnitude = 3.0, Components > +
   "Grant Tags to Target Actor" > Add to Inherited: `Cooldown.Dash`.
4. Set the ability's Cost/Cooldown GE classes, grant it on the server with an input ID.
5. Build, then test in PIE as Client with 2 players (see `unreal-multiplayer`).

## Debugging

- Console (`ue_console`): `showdebug abilitysystem` (then `AbilitySystem.Debug.NextCategory`
  / `AbilitySystem.Debug.NextTarget` to page), shows attributes, tags, active GEs and abilities.
- Gameplay Debugger (apostrophe key in PIE) has a GAS category.
- `ue_log` with filter `LogAbilitySystem|LogGameplayEffects`. For more detail run
  `log LogAbilitySystem VeryVerbose`.
- "Ability won't activate" checklist: granted on server? ActorInfo initialized on this
  machine? blocked/required tags? cost affordable? cooldown tag present? already active and
  not ended? input ID matches?

## Common pitfalls

- InitAbilityActorInfo only on server - client abilities silently fail.
- Granting abilities on the client - they are not real; grant on the server only.
- AttributeSet created on the Pawn while the ASC is on the PlayerState - ASC never finds it.
- Forgetting `REPNOTIFY_Always` or `GAMEPLAYATTRIBUTE_REPNOTIFY` - UI never updates on clients.
- Clamping Health in `PreAttributeChange` only - base still goes negative; clamp in
  `PostGameplayEffectExecute` too.
- Gameplay logic in Gameplay Cues or in `PreAttributeChange`.
- Abilities not calling `EndAbility` on montage interrupt/cancel paths.
- Using execution calculations for things that must be predicted.
- PlayerState with default net update frequency - attributes lag by up to a second.
- Hard-coded `FGameplayTag::RequestGameplayTag(FName("..."))` scattered around - typos fail
  at runtime. Use native tags.

## Verify your work

- [ ] `GameplayAbilities` plugin enabled; Build.cs has GameplayAbilities, GameplayTags, GameplayTasks.
- [ ] `ue_build` succeeds; `ue_log` shows no `LogAbilitySystem` errors on PIE start.
- [ ] InitAbilityActorInfo runs on server and owning client (add a temporary log and check both).
- [ ] `showdebug abilitysystem` in PIE (Play As Client, 2 players) shows attributes and granted abilities on the client.
- [ ] Ability activates, commits cost/cooldown, ends; cooldown tag disappears after duration.
- [ ] Tell the human which binary assets (GEs, GAs, cues) were created or changed and saved.
