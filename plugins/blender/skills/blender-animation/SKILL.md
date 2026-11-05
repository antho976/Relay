---
name: blender-animation
description: Game animation in Blender for Unreal - one action per move, keyframing pose bones from Python, loops, root motion versus in-place, timing for games, two-character interactions, props, retargeting mocap, foot sliding, baking IK, and exporting actions. Use for any keyframe, action, NLA, cycle, paired animation, mocap cleanup or animation export, and before reporting any animation as done.
---

# Animation for games

You cannot watch an animation. You write keys with bpy, then measure them with
`blender_anim_inspect` and look at still frames with `blender_render`. Load
`blender-fundamentals` first (bpy, context, files) and `blender-rigging` for the skeleton, props
and constraints. Verified snippets are in
[reference/animation-recipes.md](reference/animation-recipes.md) - read it before writing
keyframe, loop, bake, root-motion or partner code. After import, `unreal-animation-verification`
repeats these checks in Unreal (`ue_anim_inspect`, `ue_anim_preview`); `unreal-animation` covers
Animation Blueprints, montages, root motion settings and the IK Retargeter; `blender-to-unreal`
covers the export itself.

Hard rule: **never report an animation as done until `blender_anim_inspect` passes for it (with
the grips, partner and contacts of the move) and you have looked at `blender_render` images at
its key frames.** If a check cannot run, say the result is unverified.

## 1. Organisation

- **One action per move**, named `A_<Char>_<Move>` (`A_Hero_Walk`, `A_Hero_Punch_L`,
  `A_Hero_HitReact_Front`). Unreal imports each as an AnimSequence; check the imported names
  (`blender_to_unreal` takes `name`).
- **Fake user on every action** (`act.use_fake_user = True`). An action with no user is deleted
  when the file is saved and reopened.
- **Manual frame range** (`act.use_frame_range = True`, `frame_start`, `frame_end`). The exporter
  (all-actions mode), `blender_anim_inspect` sampling and loop checks use `act.frame_range`,
  which is the key range unless the manual range is on.
- **Frame rate: 30 fps** unless the project says otherwise (`scene.render.fps = 30`,
  `fps_base = 1`). Blender's factory default is 24; `blender_info` shows the file's rate. Set it
  before keying - changing fps later does not move keys. Unreal uses the file's rate on import
  unless its "use default sample rate" option is on.
- **Start every action from the rest pose.** Bones without keys in the new action keep whatever
  pose the previous action left; reset `matrix_basis` for all pose bones when you create or
  switch actions (recipe 2 does).
- **NLA is for authoring only** (layering, previewing a sequence). The exporter is run with NLA
  strips off; what exports is each action. Do not leave an action only in an NLA strip: push-down
  unassigns it, and a strip in tweak mode blocks reassigning the active action.
- One armature per character, many actions. Two characters in one file each get their own
  actions (section 6).
- **Blender 4.4+ (slotted actions):** an action has slots, and its F-curves live in a channelbag
  per slot. `action.fcurves` is a legacy view in 4.4-4.5 and is **removed in 5.0**. Reach
  F-curves only through `action_fcurves(act, obj)` from the top of
  [reference/animation-recipes.md](reference/animation-recipes.md), which works on every version.
  `pose_bone.keyframe_insert(...)` works in every version and creates and assigns the slot itself
  - prefer it where speed does not matter, and after assigning an action by code check
  `obj.animation_data.action_slot`. Bone selection moved too: `pose_bone.select` on 5.0+,
  `pose_bone.bone.select` before (recipe 6).

## 2. Keyframing from Python

- Key pose bones with `pb.keyframe_insert("rotation_quaternion", frame=f, group=pb.name)` and
  `pb.keyframe_insert("location", ...)`. Use quaternions for bones (no gimbal lock, and what the
  exporter bakes anyway); call `q.make_compatible(previous)` before keying so consecutive keys
  take the short path.
- Pose values are in the **bone's local rest axes**: Y runs along the bone, Z is set by roll.
  "Rotate the arm forward" is a different axis for each bone; test one key and measure with
  `blender_anim_inspect track` before writing a whole move.
- For hundreds of keys, write F-curves directly: `fcs = action_fcurves(act, arm, ensure=True)`,
  `fcs.new(data_path, index=i, group_name=bone)` (`action_group=` before 4.4),
  `keyframe_points.add(n)`, `foreach_set("co", ...)`, then `fc.update()` - recipe 3's `set_keys`.
- Interpolation per key: `BEZIER` with `AUTO_CLAMPED` handles for organic motion, `LINEAR` for
  root travel and mechanical parts, `CONSTANT` for stepped holds and switches (Child Of
  influence, visibility).
- Loops: first key equals last key on every channel, `act.use_cyclic = True` (cyclic-aware auto
  handles across the seam), and optionally a `CYCLES` F-curve modifier so the preview repeats.
  The FBX export bakes only the frame range, so modifiers never replace real keys.

## 3. Root motion or in-place

| | Root motion | In-place |
|---|---|---|
| What moves the character | The `root` bone's keys; Unreal moves the capsule from them (Enable Root Motion on the sequence) | Gameplay moves the capsule; the clip stays at the origin |
| Use for | Attacks with lunges, vaults, climbs, paired moves, anything whose travel must match the art exactly | Locomotion driven by input and blend spaces (the usual choice for walk/run) |
| Authoring | Animate `root` on the ground plane (x, y, yaw); keep `pelvis` relative to it | `root` stays at zero; `pelvis` bobs and sways relative to it |
| Check | Planted feet do not move in world space | Planted feet move backward at exactly the gameplay speed |

- `root` never carries height bob, lean or anything that is not the character's travel.
- Mocap and Mixamo-style data carry travel in the hips; move the ground-plane part onto `root`
  (recipe 5) or delete it for in-place.
- Record the speed (distance per cycle / cycle duration) and hand it to gameplay: a walk authored
  at 1.4 m/s played at 2.0 m/s slides.

## 4. Timing and spacing for games

Readable, responsive motion beats realism. At 30 fps:

- **Anticipation -> action -> contact -> recovery.** A light attack: 3-6 frames anticipation,
  1-3 frames to contact, 1-2 frames of contact, 8-15 frames recovery. Heavy attacks double the
  anticipation and recovery. Startup frames are gameplay: agree them with the designer; mark
  the active window for a notify (hit on/off frames).
- **Holds**: 2-4 frames on the key pose (strike, landing) so it reads at gameplay distance.
- **Spacing**: fast in the middle, slow into and out of key poses (ease); snappy attacks ease out
  hard from anticipation and ease into the hold.
- **Overlap and follow-through**: secondary parts (head, hands, tails, cloth bones) lag the
  driver by 1-3 frames and settle after it stops.
- **Weight**: heavier characters and objects take longer to start and stop; the pelvis dips on
  landing and contact.
- **Cycles**: walk ~30-36 frames per cycle (two steps), run ~18-24. Each step has contact, down,
  passing and up poses at roughly its quarters.
- **Hit reactions** start on the impact frame or one after, never before.

## 5. Props in animation

- The prop's pivot is its grip; it is parented to the hand bone, or held by a Child Of
  constraint while it changes hands (`blender-rigging`, reference/rigging-recipes.md recipe 8:
  pick-up at one frame, drop at another, no jump at either switch). Key the Child Of influence
  with `CONSTANT` interpolation.
- Throws: Child Of influence 1 -> 0 at the release frame, the prop's own keys (ballistic arc)
  after. In Unreal this becomes a detach at a notify plus a projectile; keep the release frame
  and the hand velocity in the notes.
- Props are not in skeletal exports. They ship as static meshes and attach to sockets in Unreal;
  the Blender animation is for authoring and checking the hand path.
- Check every held item with `blender_anim_inspect attachments` (grips within tolerance, `side`
  correct, clearance to the own body).

## 6. Two-character interactions

Author both characters in **one file on one timeline**, so contacts can be measured:

1. Both rigs in the file, each with its own action, same frame range and the same start frame.
2. Place them by their **object transforms** (roots at their own origins in their own actions),
   facing each other with the exact distance and angle the game will use. Recipe 7 slides the
   partner so the planned contact lands; record the resulting root distance and yaw - gameplay
   (or a motion-warping / contextual-animation setup in Unreal) must reproduce it.
3. Key the shared events on the same frames (impact, grab, release). Reactions start on the
   impact frame or later.
4. Check with `blender_anim_inspect` using `partner: {armature, action}` and `contacts`:
   ```
   blender_anim_inspect {file, armature: "Hero", action: "A_Hero_Punch", frames: [1, 8, 12, 13, 16, 24, 30],
     track: ["hand.R:tail", "partner:head"],
     partner: {armature: "Partner", action: "A_Partner_HitReact"},
     contacts: [{a: "hand.R:tail", b: "partner:head", expect: "touch", distance: 6, window: [12, 12]},
                {a: "hand.R:tail", b: "partner:head", expect: "apart", distance: 15, window: [1, 8]}]}
   ```
   Keep contact windows tight: a punch touches for 1-2 frames. In testing (recipe 7), this call
   passed with a 3 cm contact at frame 12; widening the window to [12, 16] failed at 13 and 16
   because the partner correctly recoils from frame 12 while the fist holds - fix the window,
   not the reaction. Always include the contact frames in `frames`. Any `partner_clipping`
   outside a contact is a real problem.
5. Export each character's action separately (section 9 - all-actions would mix them).

## 7. Retargeting mocap and Mixamo-style clips

Be honest: Blender has no built-in retargeter.

- **Best**: import the source clip into Unreal on its own skeleton and retarget there with the
  IK Retargeter (IK Rigs with matching chains, retarget pose to reconcile T- vs A-pose). See
  `unreal-animation`.
- **In Blender**: third-party add-ons exist; they are not bundled, so the user must install
  them. For rigs with the same bone names and rest orientations, recipe 9 (Copy Rotation per bone
  plus hips location, then bake) works.
- Before anything: compare names (`mixamorig:Hips` style prefixes), rest pose (T vs A), scale
  (clips often import with a 0.01 or 100 object scale - apply it), root (many clips have no
  `root`; the hips carry travel - recipe 5), and fps.
- After: feet (recipe 8), hands and props (`blender_anim_inspect`), and renders at key frames.

## 8. Foot contact and sliding

- Planted feet should stay put (root motion) or move at the capsule speed (in-place). Recipe 8
  prints height and slide per frame for a foot bone.
- `blender_anim_inspect` reports `feet_height` per sample and a `ground` problem when a foot is
  more than 2 cm below the rest-pose ground; add `track: ["foot.L", "foot.R"]` with explicit
  `frames` over a contact to read positions.
- Fix sliding by keying the foot with IK (target empty fixed during the contact) and baking
  (recipe 6), or by correcting root speed - not by nudging single frames.

## 9. Bake, then export

The FBX exporter samples the evaluated pose, so constraints driven by the same armature export
correctly for the current action. **Bake to keys** (recipe 6: `bpy.ops.nla.bake` with
`visual_keying=True`) when:
- constraint targets are other objects (IK empties, a partner, a prop) - their animation is not
  switched with the action;
- you export several actions from one file;
- a control rig drives an export skeleton (Rigify: `blender-rigging`
  reference/rigify-game-skeleton.md);
- twist bones or correctives are driven by constraints (they do not exist in Unreal).

Export:
- `blender_export {file, objects: ["Hero"], kind: "animation", action: "A_Hero_Walk", path}` -
  one action; the timeline is set to its range. For a skeleton that already exists in Unreal,
  `blender_to_unreal {kind: "animation", action, skeleton: "/Game/.../SK_Hero_Skeleton", destination}`.
- `all_actions: true` exports **every action whose F-curve paths resolve on the armature** - one
  take per action, named `<Armature>|<Action>`. Verified: that includes another character's
  action with the same bone names and even an IK empty's object action (`location` resolves on
  any object). Use it only in a file that holds just this character's clean actions; otherwise
  export per action.
- Skeletal exports carry deform bones only; control bones' motion must already be on deform
  bones (baked).

## Pitfalls seen in real exports

- **The UE mannequin comes into Blender at scale 0.01.** Apply the scale to the armature, its
  meshes and its actions before animating for export; `blender_export` refuses a rig whose object
  scale is not 1.
- **Keys on IK and control bones** do not export: only the evaluated pose of deform bones does.
  Bake to the deform bones (`nla.bake` with visual keying) when in doubt, and check the baked
  result with `blender_anim_inspect`; `blender_export` warns when an action keys non-deform bones.
- **Retarget source**: an animation made on one body plays wrongly on a skeleton with other
  proportions. Retarget in Unreal (IK Retargeter) instead of importing onto the other skeleton.

## 10. Verify your work

For every action, before export:

1. `blender_info {file}`: fps, action list with ranges and fake users.
2. Recipe 1: action ranges, `use_frame_range`, which actions fit which rig.
3. `blender_anim_inspect {file, armature, action, samples: 9}` - no problems. Add `track` for the
   hands, feet and props; `attachments` with `grips` for held items; `partner` and `contacts` for
   interactions.
4. Loops: recipe 4's seam check, and `blender_anim_inspect frames: [first, last]` - tracked
   positions equal (in-place) or offset by exactly one cycle's travel (root motion).
5. Feet: recipe 8 over each contact.
6. `blender_render {file, objects, action, frames: [key frames], views: ["front", "right"]}` -
   look at anticipation, contact, extreme and recovery poses. Front views mirror: the character's
   right hand appears on the image's left; trust `side` from `blender_anim_inspect`, not the image.
7. After import, `unreal-animation-verification` in Unreal.
8. Report: actions and ranges, fps, speeds for locomotion, contact frames and distances, the
   images you looked at, and the .blend files you changed.
