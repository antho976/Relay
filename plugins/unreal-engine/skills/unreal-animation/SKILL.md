---
name: unreal-animation
description: Unreal Engine 5 character animation - skeletons and import, Animation Blueprints (thread-safe update, C++ UAnimInstance), state machines, blend spaces, aim offsets, montages, notifies, anim layers, root motion, IK and retargeting, Control Rig, Motion Matching, warping, Sequencer, compression. Use for any locomotion, montage, notify, retargeting or AnimBP task, a character that slides or T-poses, or Python animation-asset scripting.
---

# Unreal animation

Animation assets (`.uasset`: Skeleton, Skeletal Mesh, Anim Sequence, Blend Space, Montage, Anim
Blueprint, IK Rig, Control Rig, Pose Search Database) are binary. You can:

- **Write C++**: `UAnimInstance` subclasses (the logic of an Anim Blueprint), `UAnimNotify` /
  `UAnimNotifyState` classes, montage playback from gameplay code, anim-related components.
- **Script assets with Python** (`ue_python`): import, list, set properties (root motion,
  compression, rate scale), add notifies/curves/sync markers via `unreal.AnimationLibrary`,
  batch retarget, create assets with factories.
- **Hand to the human**: AnimGraph and state machine wiring, blend space sample placement,
  montage section/slot layout, IK Rig chains, Control Rig graphs, Sequencer keyframing. Give exact
  click-paths and node names.

Deeper material:
- `reference/anim-instance-cpp.md` - full C++ `UAnimInstance` (thread-safe pattern), notifies,
  notify states, montage playback with callbacks, linked layers from C++. Read before writing any
  animation C++.
- `reference/locomotion-setup.md` - step-by-step third-person locomotion: assets to create,
  state machine layout, blend spaces, start/stop/pivot with distance matching, stride and
  orientation warping, foot IK, and the Motion Matching alternative. Read before building or
  fixing locomotion.

Sibling skills: `unreal-gameplay-framework` (Character, CharacterMovementComponent, input),
`unreal-gas` (ability montages, `PlayMontageAndWait`), `unreal-cpp`, `unreal-blueprints`
(Python editor scripting basics), `unreal-ai` (AI-driven movement feeding the same AnimBP).

## First checks

1. `ue_project_info`: engine version (Motion Matching, IK Retargeter and Mover differ a lot by
   version), enabled plugins (`PoseSearch`, `AnimationLocomotionLibrary`, `AnimationWarping`,
   `Chooser`, `ControlRig`, `IKRig`, `Mover`).
2. `ue_search_assets` with `class_names: ["Skeleton"]`, then `["AnimBlueprint"]`, `["AnimSequence"]`
   to see what skeletons exist (UE5 Mannequin `SK_Mannequin` is common) and which AnimBP the
   character uses.
3. Build.cs: `"Engine"` already contains `UAnimInstance`, montages and notifies. Add
   `"AnimGraphRuntime"` for anim node structs/libraries in runtime code, and plugin modules only
   when used (e.g. `"AnimationLocomotionLibraryRuntime"`, `"PoseSearch"`, `"ControlRig"`;
   confirm module names in the plugin's `.uplugin` under the engine's `Plugins/` folder).

## Core model

- **Skeleton**: bone hierarchy + sockets, virtual bones, curves, slot names, sync marker names,
  retargeting settings. Anim sequences belong to one skeleton. Several meshes can share a
  skeleton if their hierarchies are compatible (5.x also has compatible skeletons in the
  Skeleton's settings). Different skeletons => retarget with IK Retargeter.
- **Skeletal Mesh**: geometry + skin weights + reference pose; points to a Skeleton, Physics
  Asset, optional post-process AnimBP.
- **Anim Sequence**: keyed bone tracks + curves + notifies. **Montage**: sequences arranged in
  sections, played on a **slot**, for one-off actions. **Blend Space**: samples placed on 1 or 2
  axes. **Aim Offset**: additive mesh-space blend space driven by pitch/yaw.
- **Anim Blueprint**: an `UAnimInstance` subclass. **EventGraph** (game thread) and functions
  gather data; **AnimGraph** (worker thread when possible) evaluates the pose. The output pose
  feeds the mesh.

## AnimBP data flow (the rule)

Read gameplay state once per frame, store it in member variables, then let the AnimGraph read
only those members. Never call gameplay functions from AnimGraph nodes.

- Blueprint route (5.x): use **Blueprint Thread Safe Update Animation** (function override) and
  **Property Access** nodes to read owner data safely; mark helper functions Thread Safe. The
  compiler warns when a non-thread-safe call is used on the worker thread.
- C++ route: `NativeUpdateAnimation` (game thread) copies what you need; `NativeThreadSafeUpdateAnimation`
  (worker thread) derives values. See `reference/anim-instance-cpp.md`.
- Multi-threaded update needs Project Settings > Engine > General Settings > Anim Blueprints >
  Allow Multi Threaded Animation Update and the AnimBP Class Settings > Use Multi Threaded
  Animation Update (default on).
- AnimGraph node functions (On Initial Update / On Become Relevant / On Update bound on nodes,
  5.0+) must be thread safe; they are how distance matching drives Sequence Evaluators.

## State machines and transitions

- One state machine for locomotion (Idle, Start, Cycle, Stop, Pivot, Jump/Fall/Land). Keep
  actions (attack, reload, emote) out of it - play them as montages on slots layered on top.
- Transition rules read booleans/floats computed in update (e.g. `bShouldMove`, `bIsFalling`),
  not raw math. Use **Automatic Rule Based on Sequence Player in State** for Start/Stop/Land states
  that should exit when their animation ends, with a blend time.
- Conduits for shared branching; state aliases for "from any of these states" transitions.
- Blend settings: 0.1-0.25 s for locomotion, inertialization (Blend Logic: Inertialization, needs
  an **Inertialization** node after the state machine in the AnimGraph) for snappy, pop-free changes.
- Cached poses (`Save Cached Pose` / `Use Cached Pose`) to reuse the locomotion result in
  several branches (upper-body layering, IK).

## Blend spaces and aim offsets

- 1D for speed-only (Idle-Walk-Run), 2D for strafing (Direction -180..180 / Speed). Axis ranges
  must match what you feed (speed in cm/s from `Velocity.Size2D()`).
- Set **Weight Speed** / smoothing on axes to avoid jitter; sync groups or sync markers
  (`L_Foot`/`R_Foot`, added with right-click on a notify track > Add Sync Marker, or Python
  `add_animation_sync_marker`) keep
  feet in phase.
- Aim offset: poses must be **additive, Mesh Space**, base pose the idle (Additive Settings on
  each sequence). Feed pitch/yaw from `GetBaseAimRotation() - GetActorRotation()` normalized.
  Apply after locomotion, before IK.

## Montages, slots, sections, notifies

- Slots are declared on the Skeleton (Anim Slot Manager), grouped (DefaultGroup). The AnimGraph
  needs a **Slot** node for every slot you play on (e.g. `DefaultSlot` full body; `UpperBody`
  blended with **Layered blend per bone** from `spine_01`). A montage on a slot without a Slot
  node plays nothing.
- Sections split a montage (Windup / Attack / Recovery, or combo steps). Use
  `Montage_JumpToSection` and `Montage_SetNextSection` for combos.
- Montage playback: `UAnimInstance::Montage_Play` (full control, returns length, 0 on failure),
  `ACharacter::PlayAnimMontage` (convenience). Neither replicates - play on each machine
  (multicast/rep-notify, or GAS `PlayMontageAndWait` which handles prediction and replication).
- Notifies: `UAnimNotify` (instant: footstep, spawn projectile, sound) and `UAnimNotifyState`
  (window: weapon trace active, invulnerability frames). Montage notifies can be
  **Branching Point** for frame-accurate logic (e.g. combo windows); default Queued notifies
  fire on the game thread after evaluation.
- Gameplay-critical events (damage windows) that the server must see: notifies fire only when
  the mesh is ticking its pose on that machine. On dedicated servers check the mesh's
  **Visibility Based Anim Tick Option** (must tick pose when not rendered) or drive the event
  from gameplay timing instead of a notify.

## Anim layers and linked graphs

- **Anim Layer Interface** asset declares layers (e.g. `FullBody_Idle`, `UpperBody_Aim`). The main
  AnimBP calls layers; item/weapon AnimBPs implement the interface and are linked at runtime with
  `GetMesh()->LinkAnimClassLayers(WeaponAnimLayersClass)` (unlink with `UnlinkAnimClassLayers`).
  This is Lyra's pattern: one base AnimBP, per-weapon layer sets. Linked layer instances run on
  the same mesh; to read the main AnimBP's variables, add a thread-safe getter that returns the
  main instance (`GetOwningComponent()->GetAnimInstance()`, cast) and read it via Property Access.
- **Linked Anim Graph** node: embed another AnimBP class statically (sub-graph reuse).
- Prefer layers when content varies per equipped item; prefer one AnimBP when it does not.

## Root motion vs in-place

- **In-place** (default for locomotion): CharacterMovementComponent moves the capsule; the
  animation matches it (distance matching, stride warping). Responsive, network-friendly.
- **Root motion**: the animation moves the capsule. Use for attacks, dodges, vaults, traversal,
  where exact displacement matters. Enable per sequence (`Enable Root Motion`, Root Motion Root
  Lock = Ref Pose / Anim First Frame).
- AnimBP Class Defaults > Root Motion Mode: **Root Motion from Montages Only** (default and the
  only mode supported well with CharacterMovement networking). "Root Motion from Everything" is
  for single-player / non-networked characters.
- Root-motion montages in multiplayer: CMC replicates montage root motion with prediction on the
  owning client when played via GAS or on both server and client with matching timing.
- Symptom "animation moves then snaps back": root motion not enabled on the sequence, or the
  montage is not the root-motion source (mode mismatch).

## IK and procedural

- **Two Bone IK** node: limbs (hand to weapon grip, foot to ground) with an effector and joint
  target. **Leg IK** node (engine) for multi-bone legs; **Foot Placement** node (Animation
  Warping plugin, 5.2+, experimental label in early versions) for ground adaptation. Check node
  availability in the AnimGraph's right-click menu for your version.
- **IK Rig** asset (5.0+): defines retarget chains and solvers (Full Body IK, Limb, Pole). Used
  by the **IK Rig** anim node and by the IK Retargeter.
- **IK Retargeter** (5.0+): source IK Rig -> target IK Rig, chain mapping, retarget pose. Batch
  export duplicates animations for the target skeleton. The retargeter was reworked in 5.6 into a
  stack of retarget ops; menus and Python differ from 5.4/5.5 - inspect the asset UI and
  `help(unreal.IKRetargeterController)` before scripting.
- **Control Rig**: node-based rigging (Forwards Solve for procedural, Backwards Solve to bake from
  animation, Construction Event). Use in AnimBP via the **Control Rig** node for procedural
  adjustments (foot lock, look-at), and in Sequencer for keyframe animation in-editor. Graphs are
  editor work for the human; you can create the asset and list steps.

## Motion Matching and Pose Search

- Plugin **Pose Search** (plus `Chooser`, `AnimationWarping`, `AnimationLocomotionLibrary` in
  the typical setup). Assets: Pose Search Schema (which bones/trajectory features), Pose Search
  Database (the animations), and the **Motion Matching** AnimGraph node fed by a trajectory
  (Character Trajectory component or pose history node, depending on version).
- Status changed quickly across 5.3-5.6 (experimental in 5.3, promoted in 5.4 when Epic shipped
  the Game Animation Sample, APIs still moving afterwards). Check Edit > Plugins for the label in
  the project's engine version and prefer copying from the **Game Animation Sample** for that
  engine version rather than building from memory.
- Use it when: large animation sets (100+ locomotion clips), third-person action with many
  transitions. Avoid when: small indie animation set, strict memory budget, or a team that needs
  predictable hand-authored state machines. Distance matching + state machine is cheaper and
  easier to debug.

## Distance matching, stride and orientation warping

Plugins **Animation Locomotion Library** (`DistanceMatchToTarget`, `AdvanceTimeByDistanceMatching`,
`SetPlayrateToMatchSpeed`, and `PredictGroundMovementStopLocation` / `PredictGroundMovementPivotLocation`)
and **Animation Warping** (Stride Warping, Orientation Warping nodes). Start/stop animations need a
`Distance` curve, generated with the Distance Curve Modifier (an Animation Modifier). Full setup
in `reference/locomotion-setup.md`.

## Sequencer (cinematics)

- Level Sequence asset; bind actors (possessable) or spawn them (spawnable); Skeletal Animation
  tracks on the mesh; Camera Cuts track with a Cine Camera Actor; Fade and Audio tracks.
- Gameplay integration: `ALevelSequenceActor` + `ULevelSequencePlayer` (`Play`, `OnFinished`);
  for in-game cutscenes disable input and HUD during playback.
- Animation in Sequencer blends with the AnimBP only if the AnimBP has a Slot node for the
  Sequencer slot (default `DefaultSlot`) and the track uses it; otherwise Sequencer takes over
  the mesh fully while the track is active.
- Python: sequences can be created and populated (`unreal.LevelSequence`,
  `unreal.MovieSceneSequenceExtensions`, `add_possessable`, `add_track`, `add_section`); keyframing
  details are verbose - prefer handing detailed keyframing to the human. Render with Movie
  Render Queue.

## Performance and budget

- Compression: each sequence uses a Bone Compression Settings asset (recent 5.x defaults use the
  bundled ACL codec) and Curve Compression Settings. Recompress after changes; check size in the
  sequence's Details. Lower sample rate for background/NPC clips.
- **Update Rate Optimization** (URO, `bEnableUpdateRateOptimizations` on the mesh) and
  **Visibility Based Anim Tick Option** (`OnlyTickPoseWhenRendered` for NPCs that do not need
  server-side pose) cut update cost.
- **Animation Budget Allocator** plugin: `USkeletalMeshComponentBudgeted` + `a.Budget.Enabled 1`,
  `a.Budget.BudgetMs <ms>` for crowds.
- Keep AnimGraphs thread-safe so they run on workers (the game thread is the bottleneck).
- Leader pose (`SetLeaderPoseComponent`, formerly Master Pose) for modular characters instead
  of each part evaluating its own AnimBP; Copy Pose From Mesh when parts need their own post-process.
- Profile: `stat anim`, Unreal Insights with the `animation` trace channel; Rewind Debugger
  (Tools > Debug > Rewind Debugger, 5.0+) to scrub AnimBP state per frame.

## Character movement interplay

- CharacterMovement (CMC) owns velocity, acceleration, movement mode; the AnimBP reads them.
  Use **acceleration** (`GetCurrentAcceleration()`) for "wants to move" (start/stop intent) and
  **velocity** for speed. Crouch/jump state from `ACharacter` (`bIsCrouched`, `IsFalling()`).
- Rotation mode drives the animation style: `bOrientRotationToMovement` (third-person free,
  forward-only anims) vs `bUseControllerDesiredRotation` / `bUseControllerRotationYaw` (strafing,
  needs 2D blend space + orientation warping).
- Foot sliding: blend space speeds not matching CMC `MaxWalkSpeed`; fix sample speeds or use
  stride warping / `SetPlayrateToMatchSpeed`.
- **Mover** plugin (5.4+, experimental): a newer movement component; its AnimBP integration is
  different (read state from the Mover component / sync state). Check the plugin's status for
  the project version; do not migrate a CMC project to Mover without being asked.

## Python: scripting animation assets

Always verify signatures with `help(unreal.AnimationLibrary)` in the project's editor first.

```python
import unreal
# List all sequences for a skeleton
ar = unreal.AssetRegistryHelpers.get_asset_registry()
flt = unreal.ARFilter(class_paths=[unreal.TopLevelAssetPath("/Script/Engine", "AnimSequence")],
                      package_paths=["/Game/Characters"], recursive_paths=True)
skel_path = "/Game/Characters/Mannequins/Meshes/SK_Mannequin.SK_Mannequin"
for ad in ar.get_assets(flt):
    if skel_path in str(ad.get_tag_value("Skeleton")):
        seq = ad.get_asset()
        print(ad.package_name, unreal.AnimationLibrary.get_sequence_length(seq),
              seq.get_editor_property("enable_root_motion"))
```

```python
import unreal
# Add a footstep notify track + notifies, and enable root motion on an attack
lib = unreal.AnimationLibrary
seq = unreal.load_asset("/Game/Characters/Anims/Run_Fwd")
notify_cls = unreal.load_class(None, "/Script/MyGame.AnimNotify_Footstep")  # C++ UAnimNotify subclass
with unreal.ScopedEditorTransaction("Add footstep notifies"):
    if "Footsteps" not in [str(n) for n in lib.get_animation_notify_track_names(seq)]:
        lib.add_animation_notify_track(seq, "Footsteps", unreal.LinearColor(0.2, 0.8, 0.2, 1.0))
    for t in (0.12, 0.47):  # seconds
        lib.add_animation_notify_event(seq, "Footsteps", t, notify_cls)
unreal.EditorAssetLibrary.save_loaded_asset(seq)

attack = unreal.load_asset("/Game/Characters/Anims/Attack_01")
with unreal.ScopedEditorTransaction("Enable root motion"):
    lib.set_root_motion_enabled(attack, True)
unreal.EditorAssetLibrary.save_loaded_asset(attack)
```

Other useful calls (confirm with `help()`): `add_animation_notify_state_event` (seq, track,
start, duration, class), `remove_animation_notify_events_by_track`, `add_animation_sync_marker`,
`add_curve` / `add_float_curve_key`, `get_animation_notify_events`, `set_rate_scale`. Creating
montages/blend spaces: `AnimMontageFactory` / `BlendSpaceFactoryNew` with `target_skeleton`
set, via `AssetTools.create_asset`. Importing: `unreal.AssetImportTask` with `FbxImportUI`
(`import_as_skeletal`, `skeleton`, `import_animations`, `mesh_type_to_import`); in versions
where FBX goes through Interchange by default (recent 5.x), `FbxImportUI` options may be
ignored - import one file, inspect the result, then batch.

Human-only: wiring AnimGraph/state machines, placing blend space samples, montage section
layout, IK Rig chain setup, Control Rig graphs, Sequencer keyframes. Give precise steps
(node names from the right-click menu, pin connections, exact variable names).

## Common pitfalls

- T-pose in game: AnimBP class not set on the mesh, skeleton mismatch, or AnimBP compile error.
- Montage does nothing: missing Slot node, wrong slot name, or a later node overrides the pose.
- Notify fires twice / never: montage blended out before the time, Branching vs Queued confusion,
  or the pose is not ticked (server, off-screen).
- Reading `GetOwningActor()`/component state inside AnimGraph node functions: not thread safe.
- Additive aim offset looks broken: sequences not set to Mesh Space additive with the right base pose.
- Root motion with "Root Motion from Everything" in multiplayer: desync and corrections.
- Retargeted animations with bent limbs: retarget pose not matching (A-pose vs T-pose) or chain mapping wrong.


## Checking the result

Animation work is spatial and you cannot watch it play. Before reporting any animation,
attachment, retarget or two-character interaction as done, load `unreal-animation-verification`
and follow its loop: measure with `ue_anim_inspect`, look with `ue_anim_preview`, fix, repeat.
Items on the wrong side, items pointing the wrong way, grips off the palm and clipping are
caught there, not by reading the Animation Blueprint.

## Verify your work

- [ ] `unreal-animation-verification` loop run: `ue_anim_inspect` passes and `ue_anim_preview` images checked.

- [ ] C++ builds (`ue_build`) and the AnimBP compiles without thread-safety warnings (`ue_log`).
- [ ] In PIE, open the AnimBP debug (select the character instance in the AnimBP editor's debug
      dropdown) or Rewind Debugger: variables update, states transition as expected.
- [ ] No foot sliding at walk/run speeds; starts/stops/pivots do not pop.
- [ ] Montages play on the correct slot, notifies fire once, root-motion moves end where expected.
- [ ] Two-client PIE: montages/root motion look the same on the other client, no corrections.
- [ ] Changed assets saved and listed to the human; human-only graph steps listed explicitly.
