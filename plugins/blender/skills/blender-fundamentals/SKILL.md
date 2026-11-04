---
name: blender-fundamentals
description: Load first for any Blender work - the bpy data model (data versus context versus ops, data blocks, users, collections), background-mode rules and temp_override, transforms, units, modes, bmesh, modifiers, saving, and the order to use the blender MCP tools. Use whenever a task touches a .blend or bpy code, or on errors like "context is incorrect", "poll() failed" or "not in View Layer".
---

# Blender fundamentals (background mode, bpy)

You drive Blender only through the `blender` MCP server, which runs `blender -b --factory-startup`
on files in the checkout. There is no window, no viewport, no mouse and no user preferences.
Everything is Python against the data, then images you ask for.

- `reference/bpy-recipes.md` - verified snippets: create, apply transforms (data API), set
  origin, smoothing by angle, modifiers, bmesh, collections, append/link, UV channels,
  materials and textures. Read it before writing any non-trivial bpy.
- `reference/rig-recipes.md` - armature bones, bone collections, keys, automatic weights,
  applying an armature's scale together with its animation, and the Python for every
  `blender_rig_check` fix. Read it before touching a rig.

Sibling skills: `blender-modeling` (game-ready meshes), `blender-rigging` (armatures, weights),
`blender-animation` (actions, NLA), `blender-materials-baking` (materials, baking), `blender-to-unreal`
(export and import conventions). On the engine side: `unreal-fundamentals`, `unreal-animation`,
`unreal-animation-verification`, `unreal-materials-vfx`.

## What to take on, and what to push back on

Agents are good at: procedural and hard-surface modeling, kitbashing from existing parts,
cleanup (transforms, normals, UVs, naming, scale), rigging with a standard skeleton, weight
fixes, keyed and procedural animation, measuring, exporting. Agents are bad at: sculpting
organic characters or faces from scratch in code. For those, start from a base (MetaHuman, the
UE mannequin, Fab/marketplace assets, the team's base meshes) and say so to the human rather
than producing a lumpy blob.

## The tools, in order

1. **`blender_info`** without `file`: the art files in the checkout and whether Blender was
   found. With `file`: units, fps, frame range, every object (type, parent, parent bone, world
   location and size in cm, unapplied scale/rotation, modifiers), meshes, armatures, actions,
   images. Start every task here; never guess object names.
2. **`blender_python`** `{file?, code, save?, save_as?, addons?, timeout_s?}`: run bpy. Scope has
   `bpy`, `Vector`, `Matrix`, `math`, `ARGS` (the tool arguments); `import bmesh` / `os` yourself.
   Only what you `print` comes back, so print what you need to verify. An exception returns
   the traceback; nothing is saved when the code raises. `save_as` writes a new .blend (path
   relative to the checkout); `save: true` overwrites the opened file. `addons: ["rigify"]`
   enables bundled add-ons (factory settings have few on). Raise `timeout_s` (default 300) for
   bakes and heavy modifiers. Without `file` you get the factory scene: `Cube`, `Camera`,
   `Light` - delete them if you build from scratch.
3. **`blender_render`**: look. Workbench (default) is fast and shows form; `color: "RANDOM"` or
   `"OBJECT"` separates parts, `"MATERIAL"` shows slots, `"TEXTURE"` shows image textures.
   `eevee`/`cycles` for lookdev (cycles is CPU without denoiser: keep size small). `frames` and
   `action` pose a rig. `isolate` (default true) hides other meshes. The file is not changed.
4. **Checks**: `blender_mesh_check` for any mesh, measured with modifiers applied as the
   export writes it (zero-area faces, zero-length edges, inside-out normals; loose verts,
   n-gons, missing UVs as warnings); `blender_rig_check` for any rig or skinned mesh (it needs
   an armature); `blender_anim_inspect` for any action (sides, grips, clearance, contacts,
   feet). Problems must be fixed, warnings judged. Re-run after each fix.
5. **Export**: `blender_export` (FBX with Unreal's conventions) or `blender_to_unreal` (export,
   import into the running editor, measure). See `blender-to-unreal`.

Never report art as done without looking at `blender_render` images of the result and running
the relevant checks. State what you looked at and what the checks returned.

## Reading render images

- Views are relative to the character's facing (found from `.L`/`.R` bone pairs; without a rig,
  Blender's front, looking at the -Y side). **`front` mirrors**: the character's right hand is
  on the image's left, as when facing a person. Use `blender_anim_inspect` sides, not your
  reading of an image, to decide left vs right.
- `left` view = looking at the character's left side. `top` looks down Z.
- Judge silhouette, proportions, gaps, interpenetration, stretched or collapsed skin, floating
  parts, shading artifacts (black faces = flipped normals, faceting = smoothing). Render the
  same views before and after a change to compare.

## The data model

| Access | What it is | Use for |
|---|---|---|
| `bpy.data` | every data block in the file: `objects`, `meshes`, `armatures`, `materials`, `images`, `actions`, `collections`, `node_groups`... | reading and changing anything; always works in background |
| `bpy.context` | "where the user is": scene, view layer, active/selected objects, mode, area | the scene (`bpy.context.scene`), the view layer; little else in background |
| `bpy.ops` | operators = UI commands; they read context and poll it | only when there is no data-API route (apply modifier, auto weights, symmetrize, UV unwrap, export) |

- **Objects vs data.** An object (`bpy.data.objects["SM_Crate"]`) carries transform, parent,
  modifiers, constraints, vertex groups, material slot links. Its `data` (a Mesh, Armature,
  Curve, Camera...; `None` for empties) holds geometry. Two objects can share one mesh
  (linked duplicates; `me.users > 1`) - editing that mesh changes both. Names are separate:
  object `Hero` may use mesh `Cube.003`. FBX/Unreal names use the **object** name.
- **Users.** A data block with 0 users is an orphan and is **not saved**. `use_fake_user = True`
  keeps it (do this for every action not currently assigned). `bpy.data.orphans_purge(
  do_recursive=True)` deletes orphans. `bpy.data.<coll>.remove(block)` deletes one.
- **Names are unique per type.** Creating or appending a clashing name gives `Name.001`. Check
  the name you got (`ob.name`), do not assume it.
- **Linking vs appending.** Append copies data blocks into this file (editable, independent).
  Link references another .blend (read-only here, updates when the source changes; needs a
  library override to pose or edit). Use `bpy.data.libraries.load(path, link=...)`
  (recipes). For export to Unreal, linked data is fine to read but you cannot apply
  transforms or modifiers on it - append or make local first.
- **Collections** group objects; an object can be in several (`users_collection`). The scene has
  a root `scene.collection`. A new object is invisible to the scene until linked into a
  collection that is in the scene.
- **View layers** decide what is "in the scene" for selection and operators. An object in an
  excluded collection (`layer_collection.exclude`) is not in the view layer: it cannot be
  selected, made active, or entered into edit mode. Visibility: `hide_set()` (eye, per view
  layer), `hide_viewport` (global), `hide_render` (renders; `blender_export` also skips root
  objects with `hide_render` when you do not pass `objects`).

## Background-mode rules

1. **Data API first.** `bpy.data`, `object.data`, `bmesh`, `obj.modifiers.new`,
   `pose.bones[...].keyframe_insert`. They need no context and never poll-fail.
2. **Operators need context.** Set it explicitly: make the object active
   (`bpy.context.view_layer.objects.active = ob`), select what the operator acts on, and wrap
   the call in an override when it reads selections:
   ```python
   with bpy.context.temp_override(active_object=ob, object=ob,
                                  selected_objects=[ob], selected_editable_objects=[ob]):
       bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
   ```
   `temp_override` is 3.2+; the old dict-as-first-argument form was removed in 4.0.
3. **`mode_set` follows the view layer's active object**, not the override: set
   `view_layer.objects.active = ob` first (verified: an override alone left the mode unchanged).
4. **Operators that need a 3D View, UV editor or other area** (`view3d.*`, `uv.project_from_view`,
   most `screen.*`) have no area to run in. Use a data-API or bmesh equivalent.
5. **Some operators fail silently.** `transform_apply` with nothing selected raises nothing and
   does nothing. Print the state after every operator call.
6. **Check an operator before calling it**: `bpy.ops.object.transform_apply.get_rna_type().properties.keys()`
   lists its arguments; `"shade_auto_smooth" in dir(bpy.ops.object)` tells you whether this
   Blender has it (not `hasattr`, which is True for any name); `help(bpy.types.Mesh.transform)`, `dir(obj)` for the data API. Never invent an
   API name; check `bpy.app.version` when behaviour differs by version.
7. **Refresh after changing transforms**: `bpy.context.view_layer.update()` before reading
   `matrix_world`, `dimensions` or `is_negative`; `scene.frame_set(f)` before reading a pose.

## Transforms

- `location`, `rotation_euler` / `rotation_quaternion` (see `rotation_mode`), `scale` are local
  to the parent. `matrix_basis` is those (plus delta transforms); `matrix_world` is the result
  in world space. Setting `matrix_world` solves the parent for you.
- `matrix_parent_inverse` is the hidden offset stored when parenting; child world =
  `parent.matrix_world @ matrix_parent_inverse @ matrix_basis`.
- **Apply** = bake rotation and scale into the data so the object reads rotation 0, scale 1.
  Needed before rigging, before physics/collision, and before export. Two ways: the data-API
  `apply_transform` in the recipes (safe with shared data, deltas, shape keys, negative scale,
  children), or `bpy.ops.object.transform_apply` in an override. Location usually stays (it is
  where the object sits in the level) - except for assets whose pivot should be the world origin.
- **Negative scale** mirrors. Applying it turns faces inside out (the operator does not fix that
  either): flip or recalculate normals afterwards.
- **Armatures with actions**: applying scale rescales the rest pose but not bone location keys.
  Use the recipe that scales the keys too.
- **Origin (pivot)**: move the data the other way and the object to the point (`set_origin` in
  the recipes). Props: centre of the base. Held items: the grip (see `blender-to-unreal`).

## Units and scale

- Game convention: `unit_settings.system = 'METRIC'`, `scale_length = 1.0`, 1 Blender unit =
  1 m. Unreal is in cm; the exporter converts. `length_unit = 'CENTIMETERS'` only changes the
  display.
- A human is about 1.8 BU tall, a door about 2.1 x 1.0, a crate 0.5-1.0. `blender_info` reports
  sizes in cm (it accounts for `scale_length`), and `blender_rig_check` reports height.
- Files from other tools often arrive at `scale_length` 0.01 or with object scale 0.01/100.
  Decide one fix: apply the object scale so the mesh is truly metre-sized, then set
  `scale_length` 1.0 and check the size in `blender_info`. Do not mix fixes.

## Modes from Python

- `ob.mode` / `bpy.context.mode` ('OBJECT', 'EDIT_MESH', 'EDIT_ARMATURE', 'POSE', ...).
- Enter: `view_layer.objects.active = ob; bpy.ops.object.mode_set(mode='EDIT')`. Always return
  to 'OBJECT' before saving, exporting, or switching objects.
- **Edit mode holds a copy.** Mesh edits live in the edit-mesh until you leave edit mode;
  `ob.data.vertices` is stale meanwhile. **Armature `edit_bones` exist only in edit mode**;
  `data.bones` (rest, read-only positions) and `pose.bones` (pose, keys) are object/pose-side.
- **References die across mode switches.** Re-fetch `me.uv_layers[...]`, `me.vertices`,
  `edit_bones[...]` after `mode_set` (a stale UV layer read back garbage in testing).
- Pose mode is not needed to pose or key: set `pose.bones[name].rotation_*`/`location` and call
  `keyframe_insert`.

## bmesh

- Object mode: `bm = bmesh.new(); bm.from_mesh(me)` ... `bm.to_mesh(me); bm.free()`.
- Edit mode: `bm = bmesh.from_edit_mesh(me)` ... `bmesh.update_edit_mesh(me)`; never free it.
- Call `bm.verts.ensure_lookup_table()` (and edges/faces) before indexing.
- `bmesh.ops.*` take and return geometry lists: `create_cube`, `create_cone`, `extrude_face_region`,
  `inset_region`, `bevel`, `translate`, `rotate`, `scale`, `remove_doubles`,
  `recalc_face_normals`, `convex_hull`, `dissolve_degenerate`, `triangulate`, `delete`.
  `help(bmesh.ops.bevel)` shows the arguments.
- Per-face `smooth`, per-edge `smooth` (False = sharp), `seam`; UVs via
  `bm.loops.layers.uv`; deform weights via `bm.verts.layers.deform`.

## Finding and selecting

- `bpy.data.objects.get(name)` (None if absent); filter `scene.objects` by `type`, name prefix,
  collection (`collection.all_objects`), parent (`children_recursive`).
- Select without operators: `o.select_set(True/False)`, `view_layer.objects.active = o`.
- Select components (verts/edges/faces) via `me.vertices[i].select` in object mode or bmesh in
  edit mode, then `bm.select_flush_mode()`.

## Modifiers

- Add and configure with the data API: `m = ob.modifiers.new("Bevel", 'BEVEL'); m.width = 0.02`.
  Reorder with `ob.modifiers.move(from, to)`. Look at the result with
  `ob.evaluated_get(depsgraph).to_mesh()` (then `to_mesh_clear()`).
- Keep modifiers live until export: `blender_export` exports with `use_mesh_modifiers=True`, so
  Mirror, Bevel, Weighted Normal and Triangulate are baked into the FBX without touching the
  source. Apply only when a later step needs real geometry (sculpt, manual edits, weights).
- Apply one: `bpy.ops.object.modifier_apply(modifier=name)` with the object active, top of the
  stack first. Apply all: `bpy.data.meshes.new_from_object(evaluated)` (recipe). Never apply an
  Armature modifier on a mesh you still want skinned.
- 4.0: Weighted Normal and custom-normal features need `mesh.use_auto_smooth = True`. 4.1
  removed Auto Smooth; sharp edges always count and "Smooth by Angle" is a modifier.

## Non-destructive work and saving

- Default to `save_as` a new file (`art/props/crate_v2.blend`, or the project's naming) and
  keep the source untouched. Use `save: true` only when the task is to update that file, and
  say in your report that you overwrote it. `.blend` files are binary: list every one you
  wrote so the human can commit them (usually through Git LFS).
- One logical step per `blender_python` call, printed checks at the end, `save_as` on success.
  If the code raises, nothing is saved; fix and re-run from the same input file.
- Keep generated helpers (UCX, sockets, LOD copies) as separate named objects; keep modifiers
  live; keep unused actions with a fake user.
- Relative paths inside a .blend start with `//` (`bpy.path.abspath("//tex/a.png")`). Keep
  textures in the repo and relative, not absolute paths to someone's machine.

## Common errors

| Error | Cause | Fix |
|---|---|---|
| `Operator bpy.ops.X.poll() failed, context is incorrect` | the operator needs an active/selected object, a mode, or an area | set `view_layer.objects.active`, select, `temp_override(...)`; if it needs a 3D View, use the data API |
| `poll() Context missing active object` | no active object (common after loading or deleting) | `bpy.context.view_layer.objects.active = ob` |
| `Object 'X' can't be selected because it is not in View Layer 'ViewLayer'` | created with `bpy.data.objects.new` and not linked, or its collection is excluded | `collection.objects.link(ob)`; `layer_collection.exclude = False` |
| `keyword "x" unrecognized` | wrong operator argument | `bpy.ops.a.b.get_rna_type().properties.keys()` |
| `KeyError: 'bpy_prop_collection[key]: key "X" not found'` | wrong name, or `.001` suffix | `blender_info`, `bpy.data.objects.get()` |
| `AttributeError: 'Mesh' object has no attribute 'use_auto_smooth'` | 4.1+ removed Auto Smooth | branch on `bpy.app.version` |
| `ValueError: 1-2 args execution context is supported` | pre-4.0 context dict passed to an operator | `with bpy.context.temp_override(...)` |
| Edits "did not happen" | still in edit mode, stale reference, or operator had no selection | `mode_set('OBJECT')`, re-fetch, print after each step |
| Saved file lost an action/material | 0 users, not saved | `use_fake_user = True` |
| Modifier warning `Enable 'Auto Smooth'` (4.0) | Weighted Normal / custom normals | `mesh.use_auto_smooth = True` |

## Version notes (4.0 to 4.x)

- 4.0: bone collections (`armature.collections`, `bcoll.assign(bone)`) replace armature layers
  and bone groups; operator context dicts removed (use `temp_override`).
- 4.1: Auto Smooth removed (`use_auto_smooth`, `auto_smooth_angle` and
  `calc_normals_split()` are gone; sharp edges are always respected; Smooth by Angle is a
  modifier). Code that must run on 4.0 and 4.1+ branches on `bpy.app.version`.
- 4.2 LTS: EEVEE Next - the engine id is `BLENDER_EEVEE_NEXT` from 4.2 to 4.5 (it was
  `BLENDER_EEVEE`, and is again from 5.0). `enum_items` does not list the real ids; branch on
  `bpy.app.version`, or assign and read them from the `TypeError`.
  Many bundled add-ons moved to the online extensions platform, so `addons: [...]` can fail
  for those; check with `addon_utils.modules()`. Rigify and FBX stayed in the core add-ons, so
  `addons: ["rigify"]` works.

## Verify your work

- [ ] Started with `blender_info`; used the real object names.
- [ ] Each `blender_python` call printed the state it changed (names, counts, sizes, transforms).
- [ ] Transforms applied where needed (`blender_info` shows no unapplied rotation/scale).
- [ ] Sizes in cm are plausible for the object.
- [ ] Looked at `blender_render` images (front and a three_quarter at least) after the change.
- [ ] Ran `blender_mesh_check` for meshes, `blender_rig_check` / `blender_anim_inspect` for rigs
      and actions; fixed problems.
- [ ] Saved with `save_as` to a new file, or said plainly that the source was overwritten.
- [ ] Back in object mode; unassigned actions and materials have fake users.
