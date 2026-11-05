---
name: blender-to-unreal
description: The Blender to Unreal Engine 5 pipeline - the conventions both sides share (scale, axes, applied transforms, root and deform bones, SM_/SK_/A_ naming, UCX_, sockets, LODs), static, skeletal and animation FBX exports, one shared Skeleton, materials, normals, pivots, and reading blender_export and blender_to_unreal measurements. Use for any FBX export or Unreal import, or an asset that arrives too big, too small, rotated, mirrored or faceted.
---

# Blender to Unreal

Load `blender-fundamentals` first. This skill is the contract between the .blend and the
Unreal asset. Deeper material:

- `reference/troubleshooting.md` - symptom -> cause -> fix for every failure seen at the
  handoff (size, scale, facing, mirroring, bones, skinning, normals, UVs, materials,
  collision, sockets, LODs, animation, import errors). Read it when a check fails or an asset
  looks wrong in Unreal.
- `reference/prep-recipes.md` - verified bpy for UCX_ hulls, SOCKET_ empties, LOD copies, a
  held item's pivot at the grip, n-gons before tangents, comparing a rig with the shared
  skeleton's rig, and a hand-rolled FBX export. Read it before preparing an asset.

Related: `blender-rigging`, `blender-animation`, `blender-materials-baking`, `blender-modeling`
(the Blender side); `unreal-fundamentals` (asset naming, folders), `unreal-animation`
(skeletons, retargeting, root motion), `unreal-animation-verification` (sockets, grips,
attachments, sides), `unreal-materials-vfx` (materials, texture settings).

## The contract

| Topic | Blender side | Unreal side |
|---|---|---|
| Units | metric, `scale_length` 1.0, 1 BU = 1 m | 1 unit = 1 cm; a 1.8 BU human is 180 cm |
| Up | +Z | +Z |
| Character facing | faces **-Y** (Blender's front view) | skeletal mesh faces +Y in mesh space; the Character Blueprint rotates the mesh -90 yaw to face the actor's +X |
| Prop "forward" | +X | +X (actor forward) |
| Axis map (default settings) | (x, y, z) m | (100x, -100y, 100z) cm |
| Transforms | rotation 0, scale 1 on meshes, armature, collision | root bone scale 1, mesh scale 1 |
| Skeleton | one root bone (`root`) at the origin, the hips (`pelvis`) under it; deform bones only exported; no `_end` leaf bones | one root; bone names exactly as in Blender |
| Left/right | `.L`/`.R` (or `_l`/`_r`) consistently; the character's left is +X when facing -Y | same names; retarget chains and mirroring pair them |
| Names | object names = asset names: `SM_`, `SK_`, `A_` prefixes (static, skeletal, animation) | `SM_Crate`, `SK_Hero` (+ `SK_Hero_Skeleton`, `SK_Hero_PhysicsAsset`), `A_Hero_Walk` |
| Collision | `UCX_<Mesh>_NN` convex pieces (also `UBX_` box, `USP_` sphere, `UCP_` capsule) | simple collision on the static mesh |
| Sockets | empties `SOCKET_<Name>` parented to a static mesh | socket `<Name>` on the static mesh |
| LODs | `<Mesh>_LOD1`, `_LOD2` ... (the mesh itself is LOD0) | LODs, if the importer groups them (see LODs) |
| Materials | material names `M_<Name>`; slot order and names | material slots by name; `MI_` instances assigned in Unreal |
| Textures | `T_<Name>_D/_N/_ORM...`; normal maps OpenGL (+Y) | DirectX (-Y) normal maps: flip green on import |
| Pivot | props: centre of the base; held items: the grip | the actor's root / the socket snaps to it |
| Frame rate | `scene.render.fps` = the project's rate | imported at the FBX rate |

Write the project's choices (root name, prefixes, fps, item axis) into its docs once, and
follow the existing assets when they differ from this table.

## Which export

| Kind | Contains | Use for |
|---|---|---|
| `static` | the mesh and its children: `SOCKET_` empties, `UCX_`/`UBX_`/`USP_` collision, `_LODn` copies. Scale option `FBX_SCALE_NONE` | props, architecture, weapons and held items (even if they are posed on a hand in Blender) |
| `skeletal` | the armature and every mesh with an Armature modifier pointing at it; deform bones only; no leaf bones; `FBX_SCALE_ALL`; `animations: true` also bakes the current or given action | characters, creatures, anything that bends |
| `animation` | the armature's action only (no mesh) | clips for an existing Skeleton; always pass `skeleton` |

- Props parented to bones are **not** in a skeletal export. They ship as their own `SM_` and
  attach to a socket in Unreal (`unreal-animation-verification`).
- `blender_export` without `objects` takes every root mesh and armature that renders (skipping
  `hide_render` on the object or its collections) and is in the view layer. Hidden objects (eye,
  monitor, `hide_select`, on the object or its collection) are exported anyway. Objects in an
  excluded collection are left out and listed in `excluded`; name them in `objects` to export
  them. Pass `objects` to be exact.
- `action` picks the action and sets the scene range to it before baking. `all_actions: true`
  writes one take per action, fake-user actions included, so clean out stale actions first
  (Blender names takes `<armature object>|<action>`; check the imported names).
- `fbx_options` overrides any exporter setting. Do not override axes, `apply_unit_scale`,
  `apply_scale_options`, `add_leaf_bones` or `use_armature_deform_only` without a reason you
  can state; each one is a failure mode in troubleshooting.md.

## Skeletons: the root, the armature object, sharing

- **One root bone at the origin**, named as the project's skeleton expects (`root` for UE
  mannequin-compatible rigs), `use_deform` off unless weighted, the hips under it. Root motion
  is authored by animating this bone, never the armature object.
- **The armature object's name.** Blender writes the armature object as a node above the root
  bone. Unreal's importer leaves that node out when the object is named `Armature`; with another
  name it can turn into an extra root bone above `root`. Name the object `Armature` (the asset
  name comes from `name`), then confirm: `root_bone` in `blender_to_unreal` must be your root
  bone.
- **Sharing one Skeleton** (several characters, one animation set): every character has the
  same bone names and hierarchy as the rig the Skeleton came from. Extra bones (a cape, a
  ponytail) are added to the shared Skeleton on import; missing or re-parented bones are not
  fine. Compare against the source rig first (prep-recipes.md), then import with
  `blender_to_unreal {kind: "skeletal", skeleton: "/Game/Characters/Base/SK_Base_Skeleton"}`.
  Different proportions are allowed, but animations then need Unreal's retarget settings or an
  IK Retargeter (`unreal-animation`).
- **Animation-only**: `kind: "animation"`, `action: "A_Hero_Walk"`, `skeleton:` the target
  Skeleton asset. The armature's bone names must match that Skeleton. Keep one action per
  export unless you want every take.

## Meshes: smoothing, tangents, UVs, materials

- **Smoothing.** Exports use `mesh_smooth_type='FACE'` (smoothing groups) plus Blender's split
  normals. Shade smooth and mark hard edges (by angle) before export; flat-shaded meshes arrive
  faceted. Overriding `mesh_smooth_type` to `OFF` drops smoothing groups and gives Unreal's
  "No smoothing group information" warning. 4.0: sharp edges
  only split with `use_auto_smooth`; 4.1+: sharp edges always count, and the Smooth by Angle
  modifier is applied at export like any modifier. Recipe in `blender-fundamentals`.
- **Tangents** (`use_tspace`) are exported per UV map, only for meshes whose faces are all tris
  or quads: an n-gon makes the exporter warn and skip tangents for that mesh. Triangulate
  n-gons (modifier, last in the stack), and all faces when the normal map was baked from this
  mesh. In Unreal, `Import Normals and Tangents` keeps Blender's MikkTSpace.
- **UV channels.** `uv_layers[0]` is Unreal UV channel 0 (textures), `uv_layers[1]` is channel
  1. For baked lightmaps, either give each static mesh a non-overlapping `UVLightmap` as the
  second layer and point Unreal's Light Map Coordinate Index at 1, or let Unreal's Generate
  Lightmap UVs build it. With Lumen and no baked lighting it matters little.
- **Materials.** Each material slot becomes an Unreal material slot named after the Blender
  material; name them for their Unreal use (`M_Crate_Wood`), keep the count low (one draw call
  each), and do not rename slots between reimports (Unreal re-matches slots by name).
  `blender_to_unreal` imports materials and textures by default for static and skeletal
  kinds; that suits a first import that should create placeholders. Once the project has its
  own materials, pass `materials: false` and assign `MI_` instances in Unreal
  (`unreal-materials-vfx`). Image textures ship as files next to the FBX only if their paths
  resolve; embed with `fbx_options: {"path_mode": "COPY", "embed_textures": true}` when needed.
- **Normal maps.** Blender uses OpenGL (+Y green), Unreal DirectX (-Y): tick Flip Green
  Channel on the imported texture, or flip green when baking/exporting textures.

## Collision, sockets, LODs, pivots

- **UCX_ rules.** Each piece must be convex, closed and simple (tens of vertices, never
  hundreds); name `UCX_<RenderMeshObjectName>_00`, `_01` ... with the exact render mesh name;
  same space as the mesh (parent it and keep its transform applied); no modifiers or materials
  needed. A concave shape = several convex pieces. `blender_info` shows the roles it detected.
- **SOCKET_ empties** become static-mesh sockets named without the prefix. Skeletal-mesh
  sockets are made on the Skeleton in Unreal, not from empties. The FBX round trip leaves each
  socket at 100x scale with a -90 degree roll; `blender_to_unreal` fixes both (see Sockets
  under the pipeline below). A mesh imported any other way needs the same fix by hand.
- **LODs.** Unreal's classic FBX importer builds LODs from an FBX LOD Group, which Blender's
  exporter does not write. `_LODn` children therefore go into the same FBX, and what Unreal
  does with them depends on the importer and version: read `imported` and look at the LOD
  count in the Static Mesh editor. If they did not become LODs, export each LOD alone
  (`objects: ["SM_Crate_LOD1"]`) and import it into the LOD slot in the Static Mesh editor, or
  use Unreal's LOD generation / Nanite instead of hand LODs.
- **Pivots.** Props: origin at the centre of the base, so they sit on the floor at Z=0 when
  placed. Modular pieces: origin on the grid corner the kit snaps by. Held items: origin at the
  grip (where the palm closes), long axis along +X from the grip to the business end, or a
  `SOCKET_Grip` there; a second `SOCKET_OffHand` for two-handed items. This is the convention
  `unreal-animation-verification` checks grips against.

## Workflow

1. `blender_info {file}` - names, sizes in cm, unapplied transforms, roles (UCX_/SOCKET_),
   bones and actions.
2. Fix in `blender_python` (save_as a new file): apply transforms, set pivots, naming,
   collision, sockets, smoothing, UVs, materials. Print the results.
3. Look: `blender_render` (front, three_quarter; `color: "RANDOM"` to see collision and parts
   separately with `isolate: false`).
4. Check: `blender_rig_check` for skeletal assets (no problems left), `blender_anim_inspect`
   for actions (sides, grips, feet, contacts).
5. Export:
   - no Unreal editor running: `blender_export {file, objects, path: "Export/SM_Crate.fbx", kind}`
     and hand the FBX over; compare `size_cm` with what the human sees after import.
   - editor running with the Unreal plugin on: `blender_to_unreal {file, objects, kind,
     destination: "/Game/Props", name: "SM_Crate"}`.
6. Read the measurements (below), fix the cause in Blender, run the same call again. It
   replaces the asset in place.
7. Continue in Unreal with the Unreal skills (sockets and grips, materials, AnimBP) and report
   the numbers that passed.

## Reading blender_export and blender_to_unreal

- `exported.objects`, `sockets`, `collision`: exactly what went into the FBX. A missing UCX_ or
  SOCKET_ = not a child of the exported mesh, or misnamed. A prop in a skeletal list = it was
  skinned by mistake (an Armature modifier).
- `exported.size_cm` vs `imported[].size_cm` (Z compared): a **x100 or x0.01** ratio is a unit
  mistake; other ratios are an unapplied object scale or an import scale setting.
- `root_bone`: your root bone's name. The armature object's name here = the extra-root case.
- `root_bone_scale`: must be `[1, 1, 1]`. Anything else is an unapplied armature scale (often
  0.01 or 100 from an earlier import) - apply it with its animation (rig recipes) and re-export.
- `forward_axis_in_mesh_space`: expect about `[0, 1, 0]` for a character that faced -Y in
  Blender. `[0, -1, 0]` means it faced +Y, is mirrored, or has its `.L`/`.R` names swapped
  (the axis is worked out from the name pairs); `[+-1, 0, 0]` means it faced +-X. Fix
  the facing in Blender (rotate, apply), not with a rotation in Unreal.
- `hand_sides`: informational only. Sides are derived from the bone names, so they always read
  correctly; mirroring shows up in the facing instead, and `problems` reports it by comparing the
  imported facing with the facing recorded at export (`exported.forward_world`).
- `left_right_pairs`: 0 means Unreal found no pairs, so facing and sides are guesses.
- `problems` summarise the above; `import_log` holds Unreal's warnings (smoothing groups,
  tangents, bone mismatches). Read both; `passed: true` does not cover materials, collision
  shape or LODs - look at those in Unreal (`unreal-editor-automation` capture) or ask.

## Reimporting

- Keep asset names, bone names, slot names and the destination stable; `blender_to_unreal`
  with the same `destination` and `name` replaces the asset and keeps its references
  (Blueprints, levels, material assignments by slot name).
- Adding bones to a shared Skeleton is fine; renaming or re-parenting bones breaks animations
  on it. Changing the bone hierarchy of an existing skeletal mesh may fail to merge; import
  under a new name, check, then swap references.
- In the editor the human can also right-click > Reimport; it uses the FBX path stored on the
  asset, so keep exports at stable paths in the repo (`fbx_path`).

## Interchange (UE 5.5+)

Recent engines route FBX through the Interchange framework by default. The legacy
`FbxImportUI` options the tool passes may then be ignored or mapped differently (import
dialog: Common Meshes / Skeletal Meshes / Animations pipelines; settings such as Convert
Scene, Force Front X Axis and Convert Scene Unit live there). Signs: `Interchange` lines in
`import_log`, assets named differently, materials created when you asked for none. Import one
asset, inspect it, and only then batch. Leave Force Front X Axis off with these export
settings. Record in the project docs which importer and settings the project uses.

## What the handoff now does for you

- **Before export**: `blender_export` runs `blender_mesh_check` on the evaluated meshes and refuses
  to write zero-area faces (typically a bevel wider than a thin part), zero-length edges or
  inside-out normals; n-gons are a warning because the FBX exporter then skips tangents. It also
  refuses an armature whose object scale is not 1 (the UE mannequin imports into Blender at 0.01:
  apply scale to the rig, its meshes and its actions first) and warns when an action keys
  non-deform (IK or control) bones.
- **Import**: the legacy FBX importer by default, normals imported and tangents computed, import
  settings replaced on re-import, materials checked for transient instances.
- **Sockets**: `SOCKET_` empties arrive in Unreal with a -90° roll from the FBX axis conversion,
  which turns anything attached to them, and at 100x scale from the unit conversion, which
  scales it; the import divides every socket's scale back unless `socket_rotation` is `keep`. The export records how each empty is turned relative to
  its mesh, and the import sets a socket whose empty had no rotation of its own back to zero
  (`socket_rotation`: `match` by default, `zero` for all, `keep` to leave them). Empties you did
  rotate are reported for a check with `ue_screenshot`; keep socket empties unrotated and put
  the rotation on the attached item's offset where you can.
- **After import**: every mesh is rendered once and its screen coverage measured; a mesh that
  does not draw is reported, and broken results are deleted again.

**Animations onto an existing Unreal skeleton**: check the retarget source. An animation authored
on one body and imported onto a skeleton with other proportions needs the IK Retargeter (or the
right retarget source on the sequence), not a direct import; verify with `ue_anim_inspect` after
import.

## Verify your work

- [ ] `blender_info`: no unapplied rotation/scale on exported objects; sizes plausible in cm.
- [ ] Pivot where the convention says (base for props, grip for held items).
- [ ] Names: `SM_`/`SK_`/`A_`, `UCX_<Mesh>_NN`, `SOCKET_<Name>`, `_LODn`, `.L`/`.R`.
- [ ] Skeletal: `blender_rig_check` has no problems; one root; deform bones only carry weights.
- [ ] Animation: `blender_anim_inspect` passes; action frame range and fps right.
- [ ] Looked at `blender_render` images before export.
- [ ] `blender_to_unreal` (or `blender_export` plus a human import): height ratio about 1,
      `root_bone` is yours, `root_bone_scale` 1, forward about +Y, hands on their sides,
      expected assets in `imported`, `import_log` read.
- [ ] Source .blend untouched or the overwrite reported; the FBX path and Unreal destination
      reported.
