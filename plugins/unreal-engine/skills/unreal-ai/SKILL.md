---
name: unreal-ai
description: Game AI in Unreal Engine 5 - AIController, Behavior Trees and Blackboards, StateTree, EQS, AI Perception and teams, navigation and avoidance, Smart Objects, Mass, enemy design, the Gameplay Debugger and Visual Logger, AI performance. Use for any NPC, enemy, companion, patrol, chase or pathfinding task, or an AI that does not move, see or path.
---

# Unreal AI

AI in UE is split into: a **controller** that possesses a pawn (`AAIController`), a **decision
layer** (Behavior Tree + Blackboard, or StateTree), **senses** (AI Perception), **spatial
queries** (EQS), and **movement** (Navigation System + path following + the pawn's movement
component). Most bugs are in the seams between them: no nav mesh, perception not configured
for teams, BT never started, pawn never possessed.

What is text vs binary:
- Text (you write): `AAIController` subclasses, C++ BT tasks/decorators/services, StateTree
  C++ tasks/conditions/evaluators, EQS contexts/generators/tests, perception setup, team
  interfaces, `DefaultEngine.ini` navigation settings.
- Binary (Python or human): Behavior Tree, Blackboard, StateTree, EQS query assets, nav mesh
  volumes placed in levels, Smart Object definitions. BT/StateTree graph wiring is human work;
  give exact node names and order. You can create the assets and blackboard keys with Python.

Deeper material:
- `reference/behavior-tree-cpp.md` - C++ BT tasks (instant and latent), services, decorators
  and observer aborts, blackboard key selectors, node memory, attack tokens, Python to create
  BT/Blackboard assets and an example tree layout. Read before writing BT C++.
- `reference/ai-controller-perception.md` - C++ AI controller with sight/hearing perception,
  team affiliation (`IGenericTeamAgentInterface`, attitude solver), noise events, Detour crowd
  following, and running EQS queries from C++. Read before setting up perception or teams.
- `reference/statetree.md` - StateTree concepts, when to prefer it, C++ tasks/conditions/
  evaluators with instance data, running it on an AI controller, events, debugging. Read before
  building any StateTree.

Sibling skills: `unreal-gameplay-framework` (pawns, controllers, possession, GameMode),
`unreal-animation` (AI locomotion uses the same AnimBP; montages for attacks), `unreal-gas`
(abilities triggered by AI), `unreal-cpp`, `unreal-blueprints`.

## First checks

1. `ue_project_info`: plugins (`StateTree`, `GameplayStateTree`, `SmartObjects`, `MassGameplay`,
   `MassAI`). Build.cs needs `"AIModule"`, `"GameplayTasks"`, `"NavigationSystem"` for BT/AI
   work; `"StateTreeModule"`, `"GameplayStateTreeModule"` for StateTree.
2. `ue_search_assets` for `BehaviorTree`, `BlackboardData`, `StateTree`, `EnvQuery` assets and
   existing AI controllers - extend the existing approach.
3. `ue_level_actors` with `class_filter: "NavMeshBoundsVolume"` and `"RecastNavMesh"` - no volume
   means no nav mesh means no `MoveTo`.
4. Is it multiplayer? AI runs on the server only; clients see replicated movement.

## Decision layer: Behavior Tree vs StateTree

| Use Behavior Tree when | Use StateTree when |
|---|---|
| Classic enemy AI with priority-ordered behaviours (combat > investigate > patrol) | Behaviour is naturally state-based (phases, modes, scripted sequences) |
| Team already knows BTs; existing BT assets | New project on 5.3+ wanting one tool for AI, Smart Object interactions and Mass |
| You want observer aborts reacting to blackboard changes | You want a hierarchical state machine with selection by conditions and explicit transitions |
| Lots of editor tooling/debugger maturity is important | You need it for Mass Entity or Smart Object gameplay interactions (they use StateTree) |

Both are fine for shipping. Pick one decision layer per AI type; running both on the same
controller doubles the places where behaviour can be decided and makes debugging much harder.

## AIController essentials

- Pawn class defaults: `AIControllerClass = AMyAIController`, `AutoPossessAI =
  EAutoPossessAI::PlacedInWorldOrSpawned` (the default `PlacedInWorld` misses spawned enemies).
- Start logic in `OnPossess`: `RunBehaviorTree(BTAsset)` (also initializes the blackboard) or
  start the StateTree component.
- Movement: `MoveToActor` / `MoveToLocation` (returns `EPathFollowingRequestResult`), completion
  via `OnMoveCompleted` override or `ReceiveMoveCompleted`; `StopMovement()`.
- Focus: `SetFocus(Actor)` / `SetFocalPoint` make the pawn face the target when the pawn uses
  controller rotation (`bUseControllerRotationYaw` or CMC `bUseControllerDesiredRotation`);
  `ClearFocus(EAIFocusPriority::Gameplay)`.
- Character movement for AI: `bOrientRotationToMovement = true` for walkers; set
  `bUseControllerDesiredRotation` when AI should strafe while focusing a target.

Full controller code: `reference/ai-controller-perception.md`.

## Behavior Trees (short)

- **Composites**: Selector (first child that succeeds), Sequence (all children in order),
  Simple Parallel (main task + background subtree).
- **Tasks** do work: MoveTo, Wait, RunEQSQuery, PlayAnimation/montage, custom. A task returns
  Succeeded, Failed, or InProgress (latent; must later call `FinishLatentTask`).
- **Decorators** gate a branch (Blackboard condition, Cooldown, Loop, Is At Location, custom).
  **Observer aborts** (None / Self / Lower Priority / Both) make the tree react when a blackboard
  key changes - the main way a BT becomes responsive.
- **Services** tick while their branch is active (update target, distance, line-of-sight) at an
  interval with random deviation. Use them instead of ticking tasks.
- Blackboard: typed keys (Object, Vector, Bool, Float, Int, Enum, Name, Rotator, Class, String).
  Key names are strings shared by the BT and your C++ - keep them in one place.
- Structure enemy trees by priority, highest on the left: Dead/Stunned -> Combat (has target)
  -> Investigate (heard noise / last known location) -> Patrol/Idle.

## EQS

Environment Query System finds the best point/actor by generating items and scoring them with
tests: "best cover point near me, visible to nobody, reachable", "flank position", "nearest
ammo". Asset: Environment Query (`EQS_*`).
- Generators: Points: Grid / Circle / Donut / Cone / Pathing Grid, Actors Of Class, Current Location.
- Tests: Distance, Dot, Trace (visibility), Pathfinding (reachable / path length), Overlap,
  Gameplay Tags, Project (onto nav). Filter vs Score; normalize scores.
- Contexts: Querier (the AI pawn) or custom `UEnvQueryContext` subclasses returning the target.
- Run from BT (Run EQS Query task writes the result to a blackboard key) or from C++
  (`FEnvQueryRequest(...).Execute(...)`, see reference). Debug with an **EQS Testing Pawn**
  (`AEQSTestingPawn`) placed in the level, or the Gameplay Debugger EQS category.
- Cost: every item x every test. Filter cheap tests first (distance, dot), expensive ones last
  (trace, pathfinding); keep generators coarse.

## AI Perception

- Add `UAIPerceptionComponent` to the **AIController** (not the pawn) and configure senses:
  `UAISenseConfig_Sight` (SightRadius, LoseSightRadius > SightRadius, PeripheralVisionAngleDegrees
  is the half angle, DetectionByAffiliation, SetMaxAge), `UAISenseConfig_Hearing` (HearingRange),
  `UAISenseConfig_Damage`. Set the dominant sense (usually sight).
- Events: `OnTargetPerceptionUpdated(AActor*, FAIStimulus)` - check
  `Stimulus.WasSuccessfullySensed()` (false = lost sight) and `Stimulus.Type` for which sense.
  Write results to the blackboard (TargetActor, LastKnownLocation) - the BT reacts via aborts.
- Stimuli sources: by default pawns auto-register for sight. Other actors need a
  `UAIPerceptionStimuliSourceComponent` with the sense registered. Disable auto-register for all
  pawns in `DefaultGame.ini` if you want explicit control:
  ```ini
  [/Script/AIModule.AISense_Sight]
  bAutoRegisterAllPawnsAsSources=false
  ```
- Hearing needs events: `UAISense_Hearing::ReportNoiseEvent(World, Location, Loudness, Instigator, MaxRange, Tag)`
  on footsteps, gunshots. Damage: `UAISense_Damage::ReportDamageEvent(...)` where damage is applied.
- **Teams / affiliation**: sight's "Detect Enemies" only works if the AI can tell who is an
  enemy. Implement `IGenericTeamAgentInterface` on the **perceived actor** - the player pawn,
  typically forwarding to its controller or PlayerState - and set team IDs on the AI controller
  (`SetGenericTeamId`); the listener's attitude comes from the controller.
  `AAIController` already implements the interface. The default attitude solver treats different
  team IDs as Hostile and equal IDs as Friendly; an actor without the interface is Neutral.
  Either give everyone team IDs or tick Detect Neutrals too. Code in the reference.
- Sight traces from the pawn's eyes (`GetActorEyesViewPoint`); override it on the pawn to use a
  head socket. Sight uses the `Visibility` trace channel by default (configurable in the
  AISense_Sight config) - glass or foliage that blocks Visibility blocks sight.

## Navigation

- **NavMeshBoundsVolume** covers walkable space; the **RecastNavMesh** actor (auto-created) holds
  settings. Press **P** in the editor viewport to view the nav mesh (green). `ue_console` with
  `RebuildNavigation` in editor to force a rebuild.
- Agent size: Project Settings > Navigation System > Agents > Supported Agents (radius, height,
  step height) must match or exceed the capsule; otherwise paths go through gaps the character
  cannot fit. Multiple agent sizes = multiple nav meshes.
- **Runtime Generation** (Project Settings > Navigation Mesh > Runtime > Runtime Generation):
  `Static` (baked, cheapest), `Dynamic Modifiers Only` (baked, but nav modifiers/obstacles can
  change at runtime), `Dynamic` (rebuild tiles when geometry changes - doors, destructibles,
  procedural levels). For huge worlds, "Generate Navigation Only Around Navigation Invokers"
  (Navigation System settings) plus `UNavigationInvokerComponent` on AI pawns.
- **Nav Modifier Volume** / `UNavModifierComponent` with an Area Class: `NavArea_Null` (no nav),
  `NavArea_Obstacle` (high cost), custom `UNavArea` subclasses with `DefaultCost` for "avoid
  water" style costs. **Nav Query Filters** (`UNavigationQueryFilter`) change area costs or
  exclude areas per AI type (pass as FilterClass to MoveTo).
- Movable actors that should cut the nav mesh: they need `CanEverAffectNavigation` on their
  component; for moving obstacles use the static mesh's Navigation "Is Dynamic Obstacle"
  setting (acts as a modifier) together with Dynamic Modifiers Only or Dynamic generation.
- **Nav Link Proxy**: connect disconnected areas (jump down ledges, gaps). Simple links are
  point-to-point; Smart Links can notify on reach so the AI plays a jump/vault.
- Avoidance: **RVO** (`UCharacterMovementComponent::bUseRVOAvoidance`, AvoidanceWeight,
  AvoidanceConsiderationRadius) - simple, velocity-based, ignores the nav mesh edges.
  **Detour Crowd** (`UCrowdFollowingComponent` path following, e.g. via
  `ADetourCrowdAIController` or `SetDefaultSubobjectClass`) - nav-aware, better for groups.
  Use one, never both on the same agent.
- Multiplayer: nav mesh is built/queried on the server. Clients do not need it unless you do
  client-side pathing (Navigation System settings > Allow Client Side Navigation).
- World Partition: nav data can be streamed with the level (check "World Partitioned
  Navigation Mesh" in your version's RecastNavMesh settings) or use invokers.

## Smart Objects

Plugin `SmartObjects` (+ `GameplayBehaviorSmartObjects` / `GameplayInteractions` for behaviours).
A Smart Object Definition asset describes slots (sit, use terminal, lean on wall) with tags;
`USmartObjectComponent` places them in the world; AI queries the `USmartObjectSubsystem`,
claims a slot, moves there, runs the behaviour, releases the slot. Great for ambient life
(benches, vendors, workstations). The subsystem API names changed across 5.x (e.g. claim
functions were renamed); read `SmartObjectSubsystem.h` in the project's engine before writing
C++. BT (`Find and Use Gameplay Behavior Smart Object` task) and StateTree integrations exist
in the plugins above - verify node names in the editor.

## Mass Entity

Mass (`MassEntity`, `MassGameplay`, `MassAI`, `MassCrowd`, plugin-based) is an ECS for
thousands of lightweight agents (crowds, traffic, as in the City Sample). Use it when agent count
is in the hundreds to thousands and each agent's behaviour is simple. Do not use it for a
dozen hero enemies with complex combat - AIController + BT/StateTree is far simpler. Mass APIs
are still evolving between versions; always start from the engine's own examples for the version.

## Designing enemy behaviour

- **Telegraph** every damaging attack: windup animation + sound + optional VFX, long enough to
  react (0.4-1.0 s for melee in action games; longer for big hits). Put the damage window in an
  AnimNotifyState after the telegraph.
- **Fairness**: attackers are visible (no off-screen hits without warning indicators); AI
  accuracy ramps up over time instead of perfect aim; delays after losing/gaining sight
  (reaction time 0.2-0.5 s).
- **Attack tokens / slots**: a manager (world subsystem or a component on the player) grants N
  tokens; an enemy must acquire a token (BT task/decorator) before attacking and releases it after.
  Others circle, taunt, reposition. This is what makes groups feel fair and readable.
- **Readable states**: alert/search/idle should be visible (barks, animation, UI indicator).
- **Variety by parameters**: one BT/StateTree with data-driven parameters (data asset per enemy
  type: ranges, cooldowns, aggression) beats a tree per enemy.
- **Give up and reset**: AI that loses the player investigates the last known location, then
  returns to patrol. Leash distances prevent kiting enemies across the map.

## Debugging

- **Gameplay Debugger**: in PIE press the apostrophe key (`'`), look at an AI (or cycle with the
  numpad), toggle categories with numpad keys (NavMesh, AI, Behavior Tree, EQS, Perception, ...;
  the on-screen header lists them). Key can differ on non-US keyboards - it is configurable in
  Project Settings > Engine > Gameplay Debugger.
- **Behavior Tree editor** during PIE: open the BT, pick the AI in the debug dropdown - active
  branch highlights, blackboard values shown live.
- **Visual Logger** (Tools > Debug > Visual Logger, or console `VisLog`): records per-actor logs,
  shapes and BT/perception snapshots over time; scrub after the fact. Log from C++ with
  `UE_VLOG(this, LogMyAI, Log, TEXT("..."))`, `UE_VLOG_LOCATION`, `UE_VLOG_SEGMENT`.
- **StateTree debugger** (5.4+): in the StateTree editor during PIE, shows active states and
  transitions; recordings in the Rewind Debugger.
- Navigation: `P` in the viewport, `show Navigation` in PIE via `ue_console`; a path that fails
  shows in the Gameplay Debugger AI/NavMesh category.
- Log categories: `LogBehaviorTree`, `LogAINavigation`, `LogPathFollowing`, `LogEQS`,
  `LogStateTree`. Use `ue_log` with those as filter.

## Performance

- Services and BT decorators: sensible intervals (0.2-0.5 s), not every tick.
- Perception: sight is time-sliced; limit sight radius, number of listeners, and sources.
  Config knobs for traces per tick live in the `[/Script/AIModule.AISense_Sight]` config section -
  read `AISense_Sight.h` for the names in your version before setting them.
- EQS: coarse grids, cheap tests first, run on demand not on a timer per frame.
- Pathfinding: avoid re-issuing `MoveTo` every tick; MoveTo to an actor already re-paths when the
  goal moves. Hierarchical/partial paths for long distances.
- The real cost is often `CharacterMovementComponent` and animation per AI: lower tick rates for
  distant AI (significance manager), URO on meshes, simpler movement for far agents, Mass for crowds.
- Pool spawned AI instead of spawn/destroy churn.

## Common pitfalls

- AI stands still: no nav mesh under it, AutoPossessAI wrong, BT not started, MoveTo target
  not on nav (project it), acceptance radius larger than the distance.
- AI does not see the player: Detect Enemies only, but no team interface on the player; sight
  config added to the pawn instead of the controller; LoseSightRadius smaller than SightRadius.
- BT does not react: decorator observer abort set to None; key changed but the decorator
  observes a different key; task never calls `FinishLatentTask`.
- Blackboard key name typo between C++ and asset: silently invalid; log `IsValid` on key IDs.
- AI logic running on clients: guard with `HasAuthority()`; AIControllers exist only on the server.

## Verify your work

- [ ] `ue_build` succeeds; `ue_log` filtered on `LogBehaviorTree|LogStateTree|LogAINavigation|Error` is clean.
- [ ] Nav mesh visible (P) where AI must walk; agent radius/height match the capsule.
- [ ] Gameplay Debugger shows the expected BT branch / StateTree state, perception stimuli and path.
- [ ] Every attack is telegraphed, and at most N enemies attack at once (tokens).
- [ ] Lose-sight and give-up behaviour tested; AI returns to patrol.
- [ ] Tested with several AI at once and in a listen/dedicated server PIE session.
- [ ] Assets created/changed are saved and listed for the human, with any graph-wiring steps.
