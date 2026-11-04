---
name: blender-rigging
description: Rig and skin characters and props in Blender for Unreal - armature structure and naming, matching the UE mannequin, Rigify to a clean game skeleton, weights and their cleanup, sockets and held props, twist bones and corrective shape keys. Use for any armature, skinning, weight, bone, socket or attachment work, and when a rig deforms, twists, mirrors or exports wrong.
---

# Rigging and skinning for games

You drive Blender only through the `blender` MCP tools, in background mode. You cannot drag a
bone or paint a weight, so everything here is done with bpy and checked with numbers
(`blender_rig_check`, `blender_anim_inspect`) and pictures (`blender_render`). Load
`blender-fundamentals` first for bpy, context overrides and file handling; `blender-animation`
for actions; `blender-to-unreal` for export and import. Verified snippets are in
[reference/rigging-recipes.md](reference/rigging-recipes.md) - read it before writing any
armature, weight or constraint code. For Rigify characters that ship to Unreal, read
[reference/rigify-game-skeleton.md](reference/rigify-game-skeleton.md).

Hard rule: **never report a rig or skin as done until `blender_rig_check` has no problems and you
have looked at `blender_render` images of the mesh in rest pose and in extreme poses.**

## 1. Decide first: which skeleton

| Situation | Do this |
|---|---|
| Character should use Unreal's existing animations (mannequin locomotion, Lyra, marketplace packs) | Skin to the **UE mannequin skeleton itself** (export `SKM_Manny`/`SK_Mannequin` from Unreal as FBX, import it, keep its bones), or build a skeleton with matching names and hierarchy and retarget with the IK Retargeter. Section 3. |
| Custom creature, animations authored in Blender | Your own skeleton, built to the rules in section 2. Rigify is fine for authoring if you also make a clean export skeleton (section 4). |
| Prop that animates (door, chest, gun slide) | Small armature: `root` + one bone per moving part, rigid weights (1.0 per part). |
| Prop that is only held or worn | No armature. Pivot at the grip, attach in Unreal to a socket (section 6). |

Start characters from a base mesh or the mannequin. Do not try to sculpt an organic character
in code; say so if asked.

## 2. Armature rules for Unreal

- **One root bone at the origin**, named `root`, head at (0,0,0), everything parented under it.
  Unreal takes the first bone as the root; two top-level bones are a hard failure
  (`blender_rig_check` flags it). The root is where root motion lives.
- **Armature object named `Armature`, at the origin with identity transform** (location 0,
  rotation 0, scale 1). Another object name can become an extra root bone in Unreal
  (`blender-to-unreal`); the examples here use `Hero` only for readability.
  Apply scale and rotation to the armature *and its meshes together* (recipe 2). An unapplied
  0.01 or 100 scale is the classic "character is 100x" / "root bone scale is not 1" import.
- **Character faces -Y, Z up, feet on Z=0.** Rest pose is a clean T- or A-pose.
- **Deform vs control.** `bone.use_deform = True` only on bones that carry vertex weights (and on
  empty "socket" bones you want exported, see 6). IK targets, poles, controllers and mechanism
  bones are non-deform. `blender_export` writes deform bones only (plus any non-deform
  ancestors of deform bones, which is how a non-deform `root` still exports). Non-deform bones
  with no deform children are dropped.
- **Consistent bone axes.** Blender bones point along local +Y; roll sets local Z. Pick one
  convention (e.g. Z points forward/up for arms, forward for legs) and hold it on both sides.
  Recalculate roll in edit mode with `bpy.ops.armature.calculate_roll(type=...)` on selected
  bones; check mirrored rolls with the mirror report in recipe 4 (`blender_rig_check` only checks
  head positions, not roll).
- **Naming.** Side suffixes Blender understands: `.L/.R`, `_L/_R`, `_l/_r`, `-L/-R`
  (`bpy.utils.flip_name("hand_l") == "hand_r"`). Use `.L/.R` for your own rigs, `_l/_r` when
  matching the UE mannequin. No spaces. No leftover `_end` leaf bones.
- **Symmetrize, do not hand-build the second side.** Build the left side, select it, run
  `bpy.ops.armature.symmetrize(direction="NEGATIVE_X")` in edit mode (recipe 3). It mirrors
  position, roll, parenting and constraints.
- **Bone collections (4.0+).** Armature layers and bone groups are gone. Use
  `arm.data.collections.new("DEF")`, `coll.assign(bone)`, `coll.is_visible`. In 4.1+
  collections nest; `arm.data.collections` lists top-level ones only, `collections_all` lists all.
  Collections are for organisation and visibility; they do not affect export.
- **Edit-bone and Bone references go stale** across `mode_set` calls. Store names or copied
  vectors, never `Bone`/`EditBone` objects, across a mode switch.
- **`Bone.x_axis/y_axis/z_axis` are relative to the parent.** For armature-space axes use
  `bone.matrix_local.col[i].to_3d()`.

Game budgets: 50-80 deform bones for a hero with fingers, 20-40 for crowds. Influences per
vertex: 8 is a safe cap for PC/console, 4 for mobile and low LODs (Unreal can take more, but
every extra influence costs skinning time). Twist bones count toward the bone total.

## 3. Matching the UE mannequin

Unreal's shipped animations play on your mesh directly only if it uses the **same Skeleton
asset** (same bone names, hierarchy and reference pose). Otherwise you retarget.

UE5 mannequin (Manny/Quinn) core hierarchy:

```
root
  pelvis
    spine_01 > spine_02 > spine_03 > spine_04 > spine_05
      neck_01 > neck_02 > head
      clavicle_l > upperarm_l > lowerarm_l > hand_l   (+ upperarm_twist_01/02_l, lowerarm_twist_01/02_l)
        hand_l: thumb_01..03_l, index_metacarpal_l > index_01..03_l, middle_..., ring_..., pinky_...
      (same for _r)
    thigh_l > calf_l > foot_l > ball_l   (+ thigh_twist_01/02_l, calf_twist_01/02_l)
  ik_foot_root > ik_foot_l, ik_foot_r
  ik_hand_root > ik_hand_gun > ik_hand_l, ik_hand_r
  interaction, center_of_mass
```

UE4's mannequin has spine_01..03, one neck, one twist bone per segment and no metacarpals.
UE5 also has corrective helper bones; take the real list from the asset, not from memory.

Options, most to least reliable:

1. **Rig to the mannequin skeleton.** Export the mannequin skeletal mesh from Unreal (FBX), import
   it with `bpy.ops.import_scene.fbx(filepath=..., ignore_leaf_bones=True,
   automatic_bone_orientation=False)`, delete its mesh, fit your mesh to its proportions (scale
   the mesh, not the bones), and skin to it. Keep bone orientations as imported - automatic bone
   orientation breaks sharing the skeleton. An imported cm-based FBX often arrives with a 0.01 or
   100 object scale; apply it with its children, then round-trip with `blender_to_unreal`
   (`skeleton` set to the mannequin's Skeleton) and confirm height and root scale 1.
2. **Same names and hierarchy, your own proportions.** Unreal can share one Skeleton between
   meshes with different proportions (retarget options per bone), but it is fragile; use the IK
   Retargeter instead unless the team already does this.
3. **Own skeleton + IK Retargeter** in Unreal (IK Rig per skeleton, chains, retarget pose). The
   most forgiving route for creatures and stylised proportions. See `unreal-animation`.

Rest pose: UE mannequins use an A-pose-like reference pose; Mixamo uses a T-pose. A mismatch is
fixed in the IK Retargeter's retarget pose, not by re-rigging. Match proportions to the
source animations you care about (arm length decides whether hands reach the same targets).

## 4. Rigify

Rigify is bundled; Blender runs with factory settings, so enable it per call:
`blender_python {addons: ["rigify"], code: ...}`.

1. `bpy.ops.object.armature_human_metarig_add()` adds the human metarig (`metarig`), in a
   roughly human size at the origin. Fit its bones to the mesh (edit bones by code, then check
   with renders).
2. Before generating, set `pose.bones[n].rigify_parameters.bbones = 1` on every metarig bone
   that has a Rigify type: Unreal has no B-Bones, so curved B-Bone deformation in Blender will
   not match the game.
3. Generate: make the metarig active and selected and call `bpy.ops.pose.rigify_generate()`
   **without** `temp_override` - Rigify switches the active object internally and a pinned
   override makes it fail with an AssertionError. The result is `rig` (also
   `metarig.data.rigify_target_rig`).
4. Skin to `rig` with automatic weights; only `DEF-` bones are deform, so only they get weights.

**Exporting a game skeleton from Rigify - be honest about it.** The generated rig has ~700
bones, several top-level bones, and its `DEF-` bones are parented to `ORG-`/`MCH-` bones, not to
each other. A deform-only FBX export therefore drags the non-deform ancestors along and the
hierarchy is not a clean game skeleton. Options:

- **Clean export skeleton (recommended, verified):** build a separate armature from the `DEF-`
  bones with a proper hierarchy and a `root`, make each bone follow its `DEF-` bone with Copy
  Transforms, re-target the mesh to it, and bake actions onto it
  ([reference/rigify-game-skeleton.md](reference/rigify-game-skeleton.md)). Export that armature
  with the constraints muted.
  Keep face bones out unless the game needs them.
- **Third-party add-ons** (e.g. game-rig tools for Rigify) automate the same idea; they are not
  bundled, so the user must install them. Do not claim they are available.
- **Export the Rigify rig as is:** only for quick tests. Expect extra bones and a messy
  hierarchy.

## 5. Skinning

- **Automatic weights** (bone heat): select the mesh(es) and the armature, armature active, and
  run `bpy.ops.object.parent_set(type="ARMATURE_AUTO")` in a `temp_override` (recipe 5). It
  weights deform bones only. It fails on meshes with overlapping or disconnected shells, non-
  manifold or very small geometry ("Bone Heat Weighting: failed to find solution"): merge by
  distance, skin separate parts separately, or weight rigid parts by assignment. Always count
  unweighted vertices afterwards.
- **Rigid parts** (armour plates, props, robots): assign whole vertex sets to one group at 1.0
  with `vg.add(indices, 1.0, "REPLACE")` - more reliable than heat.
- **Cleanup, in this order**: mirror (if built symmetric) -> strip wrong-side weights -> clean
  (< 0.01) -> limit total (4 or 8) -> normalize all. Mirroring or stripping after normalizing
  breaks the sums. Recipes 6 and 7 do this with operators and with the data API.
- `vertex_group_clean` and `vertex_group_limit_total` act on the objects **selected in the view
  layer**, not on the override's `selected_objects` (verified: with only an override they report
  FINISHED and change nothing). `select_set(True)` the mesh, deselect the rest, then call them.
  Re-count influences afterwards instead of trusting `{'FINISHED'}`.
- Remove vertex groups for non-deform bones and groups with no bone.
- Keep the mesh's transforms applied and the Armature modifier first in the stack (before
  subdivision or anything that changes vertex count).

## 6. Sockets and held props

- **Model props with the pivot at the grip** (the point inside the closed hand), long axis along
  a documented axis (e.g. blade along +X). `blender_anim_inspect` measures item ends from the
  object's bounding box; `blender-modeling` covers pivot placement.
- **Skeletal-mesh sockets are made in Unreal** (Skeleton editor or `ue_python`), not imported from
  FBX empties - `SOCKET_` empties only become sockets on static meshes. To carry a grip position
  from Blender, either add a bone with `use_deform = True` and no weights (exported like the
  mannequin's `ik_hand_gun`), or record the offset from `hand_r` and create the socket in Unreal.
  A non-deform socket bone is dropped by the deform-only export.
- **Preview in Blender:** parent the prop to the hand bone (`parent_type = "BONE"`), then set
  `matrix_world` to where it should sit - bone-parented children are placed relative to the
  bone's **tail**, so do not set `location` directly (recipe 8).
- **Pick-up / drop:** Child Of constraint targeting the hand bone, influence keyed 0 -> 1 at the
  pick-up frame with the inverse matrix set at that frame; at the drop frame key the object's
  own transform to its visual transform and influence 1 -> 0 (recipe 8, verified: no jump at
  either switch). In Unreal the same thing is an attach/detach at a notify; see
  `blender-animation` and `unreal-animation-verification`.
- `blender_export` skeletal exports do **not** include props parented to bones. Export props as
  static meshes; attach them in Unreal.

## 7. Twist bones and correctives

- **Candy-wrapper twisting** (forearm or upper arm collapses when the hand rotates) means one bone
  carries all the twist. Add twist bones along the segment (UE: `lowerarm_twist_01_l`), weight the
  forearm in bands, and drive each with Copy Rotation, Y only, LOCAL space, influence 0.5
  (recipe 10). The driving constraints do not export: bake them into each action
  (`blender-animation`), or drive twist in Unreal (the mannequin does it in its post-process
  AnimBP / Control Rig).
- **Corrective shape keys** fix elbows, knees and shoulders at extreme angles: a shape key per
  pose, driven by the bone's rotation (driver variable `TRANSFORMS`, `ROT_X`,
  `SWING_TWIST_Y`, `LOCAL_SPACE`). Keep the expression simple (`min`, `max`, arithmetic) so it
  runs with Python auto-exec off. Shape keys export as morph targets; drivers do not - drive the
  morph in Unreal (Pose Driver / curves) and check it there.

## 8. Common failures

| Symptom | Cause | Fix |
|---|---|---|
| Moving one leg drags the other | Weights bleed across the crotch (heat) | Strip `thigh/shin.L` weights from verts with x < 0 and vice versa (recipe 7), then clean/normalize |
| Wrist or forearm collapses when the hand twists | No twist bones | Section 7 |
| Mirrored pose bends the other side the wrong way | Rolls not mirrored; right side built by hand | Delete the right side, symmetrize; run the mirror report |
| Import is 100x or root bone scale 0.01 | Scale not applied (armature or meshes) | Apply rotation+scale to armature and meshes together, re-export |
| Extra root bone named after the armature in Unreal | Armature object not named `Armature`, so its FBX node becomes a bone | Name the armature object `Armature` (`blender-to-unreal`); verify with `blender_to_unreal` |
| Vertices stay behind at the origin | Unweighted verts | Count them; weight or merge |
| Spiky deformation | Tiny stray weights on far bones | Clean < 0.01, limit total, normalize |
| Mesh doubles/offsets when posed | Mesh parented with an unapplied or non-identity offset | Apply the mesh transform, re-parent (keep transform) |
| Prop in the wrong hand in Unreal | Front renders mirror; `.L/.R` confused | Use `blender_anim_inspect` `attachments[].side`, not the image |

## 9. Verify your work

1. `blender_info {file}`: one armature, bone count, deform bones, roots = 1, left/right pairs,
   no unapplied transforms.
2. `blender_rig_check {file}`: no problems; read every warning (unweighted verts, >8
   influences, weights on non-deform bones, asymmetric pairs, facing).
3. Mirror report (recipe 4) for rolls.
4. `blender_render {file, views: ["front", "right", "three_quarter"]}` in rest pose, then pose
   extremes (arms up, deep squat, twisted wrist, head turned) with a test action
   (`action`, `frames`) - look for bleeding, collapsing joints, spikes.
5. Props: `blender_anim_inspect` with `attachments` and `grips`; `side` must match the design.
6. `blender_export` / `blender_to_unreal kind: skeletal`: height matches, root scale 1, hands on
   the right side.
7. Report the numbers (height cm, bone count, max influences) and which images you looked at.
   Name the .blend files you changed.
