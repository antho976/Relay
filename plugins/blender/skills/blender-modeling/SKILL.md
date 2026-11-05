---
name: blender-modeling
description: Game-ready modeling in Blender from Python for Unreal - blockouts, hard-surface props, modular kits, modifiers and geometry nodes, booleans and cleanup, normals, UV unwrapping and lightmap UVs, LODs, UCX_ collision, polycount and scale checks. Use whenever you create, edit, clean, UV, LOD or measure a mesh in a .blend.
---

# Game-ready modeling in Blender (agent edition)

You drive Blender only through the `blender` MCP server (background `blender -b`). You cannot
see a viewport, so every change is: script with `blender_python` -> measure with `blender_info`
-> look with `blender_render` -> fix. Verified snippets for everything below are in
[reference/modeling-recipes.md](reference/modeling-recipes.md). Read it before you write
modeling code; copy from it instead of inventing API calls.

Related skills: `blender-fundamentals` (bpy, context, units, saving), `blender-materials-baking`
(materials, bakes, texture sets), `blender-rigging` (anything that deforms),
`blender-to-unreal` (export and import). Unreal side: `unreal-fundamentals`,
`unreal-level-environment`, `unreal-materials-vfx`, `unreal-performance`.

## 1. Know what you are good and bad at

Do it (reliable from code):
- Blockouts and greyboxes at exact dimensions; kit pieces on a grid.
- Hard-surface props: crates, doors, pipes, panels, railings, weapons made of primitives,
  bevels, booleans, arrays, mirrors, screws.
- Modular kits (walls, floors, trims, stairs) with exact snapping and pivots.
- Procedural: Array/Mirror/Bevel/Solidify/Screw/Boolean stacks, geometry nodes (scatter,
  instancing, parametric shapes).
- Kitbashing: append parts from existing .blend files, join, clean, re-UV.
- Cleanup and prep: merge by distance, loose geometry, normals, n-gons, scale, pivots, UVs,
  LODs, collision, naming.

Do not attempt (tell the user, propose an alternative):
- Sculpting organic characters, faces, creatures, cloth folds from scratch. Start from a base:
  MetaHuman, the UE Mannequin, Fab/marketplace assets, the team's base meshes, or a scan.
  You may fix, retopo-assist (decimate, shrinkwrap, remesh for a proxy), rig and export them.
- Hand-painted or artistic detail that needs an eye. Offer a procedural stand-in and flag it.
- Retopology of a high-res sculpt to production quality. You can produce a usable Decimate or
  Remesh proxy for LODs/collision; say it is not animation-grade topology.

## 2. Scale, units, orientation (check first, every time)

- 1 Blender unit = 1 m, `scene.unit_settings.scale_length == 1.0`. Unreal is cm, the FBX export
  converts. `blender_info` reports dimensions in cm - use those numbers in reports.
- Z up, front faces -Y. Apply rotation and scale before export (`blender_info` flags unapplied
  transforms). Keep location at the pivot you want in Unreal.
- Sanity heights (cm): door 200-220 high x 90-120 wide, wall 300-400, floor-to-floor 300-400,
  stair step 15-20 rise x 25-30 run, table 75, chair seat 45, counter 90, railing 100-110,
  human 175-185. UE's default character capsule is radius 42, half-height 96 (84 cm wide,
  192 cm tall): doorways and corridors must clear it with margin.
- After setting `location`/`parent` in a script, `bpy.context.view_layer.update()` before reading
  `matrix_world` or world bounds; it is stale until then (verified).

## 3. Polycount budgets (triangles, LOD0, guidance - confirm with the project)

| Asset | PC/console (non-Nanite) | Mobile/Switch |
|---|---|---|
| Small prop (cup, bottle, tool) | 100 - 1.5k | 50 - 500 |
| Medium prop (chair, crate, barrel) | 500 - 5k | 200 - 1.5k |
| Large prop (car, machine, tree trunk) | 10k - 60k | 3k - 10k |
| Modular wall/floor piece | 50 - 2k | 12 - 500 |
| First-person weapon | 10k - 40k | 3k - 10k |
| Hero character (skinned) | 30k - 100k | 5k - 20k |
| Background character / NPC | 10k - 40k | 2k - 8k |

- UE5 Nanite (static, opaque/masked meshes) removes most LOD and triangle pressure; still
  keep disk size sane and do not ship unused interior faces. Nanite does not cover skeletal
  meshes on UE 5.0-5.4, translucent materials, or mobile - those need real budgets and LODs.
- Count triangles, not faces: `blender_info` reports triangles; in code use
  `mesh.calc_loop_triangles(); len(mesh.loop_triangles)` on the evaluated mesh.
- Spend triangles on silhouette. Flat areas get none; detail goes to the normal map.

## 4. Topology rules

Static props: triangles and n-gons are fine on flat planar faces; avoid long thin slivers and
n-gons that are concave (bad triangulation, shading artifacts). Delete faces never seen
(bottom of a crate on the ground, backs of wall pieces that touch other pieces) when the
piece is never rotated to show them.

Deforming meshes (characters, cloth, hoses, anything skinned):
- All quads in deforming areas, even edge flow following muscles.
- 3 edge loops at elbows/knees/fingers (one at the joint, one either side); 2-3 around
  shoulders and hips, plus a loop pair for the wrist and ankle.
- No n-gons, no poles on joints, consistent density; loops around eyes and mouth.
- Hand those meshes to `blender-rigging` for weights and checks.

## 5. Modular kits

- Pick a grid (UE snapping is 10/50/100 cm). Kit sizes are grid multiples: 100, 200, 400 cm.
- All piece extents land exactly on the grid; verify vertex X/Y/Z are multiples of the grid.
- Pivot at a consistent corner at floor level (for walls: base corner, the wall running along +X),
  or base center for props. Set it by transforming mesh data (recipe: set_origin), not by eye.
- Wall thickness constant across the kit (e.g. 20 cm) and the wall's face on the grid line or
  centered on it - decide once, write it in the kit notes, keep every piece consistent.
- Name: `SM_<Set>_<Piece>_<W>x<H>[_Variant]`, e.g. `SM_Dungeon_Wall_400x300_A`.
- Keep bevels off the edges that touch neighbors, or seams show; bevel only exposed corners.
- Render several pieces side by side with `blender_render` (color RANDOM) to check joins.

## 6. Hard surface: bevels and weighted normals

- Bevel modifier: `limit_method='ANGLE'` (30 deg), `width` 0.5-2 cm for props, `segments` 1-3,
  `harden_normals=True`. Follow with a Weighted Normal modifier (`mode='FACE_AREA'`,
  `keep_sharp=True`). Faces must be shaded smooth.
- Blender 4.0 and earlier: custom normals (harden normals, weighted normals) only work with
  `mesh.use_auto_smooth = True`. 4.1+ removed `use_auto_smooth`; custom normals always apply
  and sharp edges come from the `sharp_edge` attribute. Guard with `hasattr(mesh, "use_auto_smooth")`.
- Unreal must keep them: static mesh import "Normal Import Method" = Import Normals (or
  Import Normals and Tangents), not Compute Normals.
- Floating decals and support loops are optional; bevel + weighted normals is the default.

## 7. Smoothing by version

- All versions: `mesh.polygons.foreach_set("use_smooth", [True]*n)` then mark sharp edges.
- 4.0: `mesh.use_auto_smooth = True; mesh.auto_smooth_angle = radians(30)` (angle-based), or
  `bpy.ops.object.shade_smooth(use_auto_smooth=True, auto_smooth_angle=...)` with a context.
- 4.1+: auto smooth is gone. `bpy.ops.object.shade_smooth_by_angle(angle=...)` marks sharp
  edges once; "Smooth by Angle" is a geometry-nodes modifier asset (the UI's Shade Auto
  Smooth adds it). Old files that used auto smooth get that modifier on load. The FBX exporter
  evaluates it. These 4.1+ paths are not runnable on the 4.0.2 used to verify this skill:
  check the operator exists and read its `get_rna_type().properties.keys()` before calling.
- Portable (verified on 4.0, correct on 4.1+ by design): mark `edge.smooth = False` by face
  angle in bmesh and set faces smooth (recipe: sharp_by_angle).
- Check an operator exists with `"shade_smooth_by_angle" in dir(bpy.ops.object)`.
  `hasattr(bpy.ops.object, name)` is always True - do not use it (verified).

## 8. Booleans

- Boolean modifier, `solver='EXACT'`, cutter object `display_type='WIRE'`, `hide_render=True`.
  Keep it live while iterating; apply before UV/bake/export for final assets.
- Apply without operators: `bpy.data.meshes.new_from_object(obj.evaluated_get(depsgraph))`
  and swap `obj.data` (recipe: apply_all_modifiers), or `bpy.ops.object.modifier_apply` under
  `temp_override(object=..., active_object=...)`.
- After a boolean always: merge by distance, dissolve degenerate, recalc normals, then look
  for non-manifold edges and concave n-gons; triangulate or add supporting cuts where
  shading breaks. Remove the cutter objects or keep them in a hidden `CUTTERS` collection.
- To control triangulation of n-gons yourself, end the stack with a Triangulate modifier
  (`quad_method='BEAUTY'`, `ngon_method='BEAUTY'`; on 4.0 also `keep_custom_normals=True`,
  an option 4.1+ no longer needs - guard with `hasattr`).
- Booleans on non-manifold or overlapping input produce garbage; clean inputs first.

## 9. Mirror and symmetry

- Model half, Mirror modifier on X with `use_clip=True`, `use_mirror_merge=True`,
  `merge_threshold` 0.1 mm. The mirror plane is the object's origin - keep it on the center line.
- To make an existing mesh symmetric: `bmesh.ops.symmetrize(bm, input=..., direction='-X')`
  copies the -X half onto +X.
- Mirrored UVs overlap: fine for base color and normals (UE handles mirrored tangents), not
  for lightmap UVs and not for unique bakes like AO - offset one side by 1 UDIM (U+1) when baking,
  or unwrap after applying the mirror.

## 10. Cleanup (before UV, bake, export)

Run the recipe `clean_mesh`: remove_doubles (merge by distance, 0.01-0.1 mm), dissolve
degenerate, delete loose verts/edges, recalc outward normals. Then report non-manifold
edges, zero-area faces, n-gons, loose verts (`mesh_report`). `blender_mesh_check {file,
objects}` checks the mesh as it will be exported, modifiers applied: zero-area faces (a bevel
wider than a thin part), zero-length edges and inside-out normals are problems; loose verts,
n-gons and missing UVs are warnings. `blender_export` runs the same check and refuses a broken
mesh.

## 11. UVs

- UV0 (first layer) = textures. UE's first UV channel is UV0; order in `mesh.uv_layers` is
  the order in Unreal.
- Fast: `bpy.ops.uv.smart_project(angle_limit=radians(66), island_margin=0.02)` in Edit Mode
  (needs active object + mode_set; recipe: smart_uv). Good for props and kit pieces.
- Controlled: mark seams in bmesh (`edge.seam = True`) on hard edges and hidden places, then
  `bpy.ops.uv.unwrap(method='ANGLE_BASED', margin=...)`, then
  `bpy.ops.uv.pack_islands(margin=..., rotate=True)`.
- Texel density: pick one per project (e.g. 5.12 px/cm = 512 px/m for third-person, 10.24 for
  first-person hero props) and hold every asset to it. Measure with `texel_density`, fix with
  `scale_uvs_to_density`, then repack if it leaves 0-1 (tiling surfaces may exceed 0-1).
- Lightmap UV (only for baked Lightmass lighting; Lumen does not need it): second layer, no
  overlaps, generous margin, in 0-1. Or leave it to UE's "Generate Lightmap UVs" (default on for
  static meshes) which writes UV1 from UV0. If you author it, disable generation on import and
  set Light Map Coordinate Index = 1.
- Keep seams where normal map breaks are hidden; split UVs on every hard (sharp) edge so baked
  normals do not smear.

## 12. LODs

- Make LODs from the final LOD0: copy, Decimate `COLLAPSE` with ratios like 1.0 / 0.5 / 0.25 /
  0.1 (or target each LOD at ~50% of the previous), `use_collapse_triangulate=True`, apply.
  Collapse keeps UV layers (verified); re-check shading afterwards (re-run sharp_by_angle /
  Weighted Normal on the LOD if needed) and the silhouette with `blender_render`.
- Names `<Mesh>_LOD0`, `<Mesh>_LOD1`, ... on the same pivot. `blender_export` (static) includes
  `_LODn` children, so parent LOD1+ to LOD0 (the recipe `make_lods` does); confirm the LOD
  count on the Unreal asset after import (`blender_to_unreal`
  reports what arrived). If they arrive as separate meshes, import each LOD file onto the
  base mesh in Unreal (see `unreal-editor-automation`) or use UE's auto LOD reduction.
- Planar decimation (`decimate_type='DISSOLVE'`) is for hard-surface cleanup, not LODs of
  organic shapes. Skip LODs for Nanite meshes unless the project needs fallback meshes.

## 13. Collision (UCX_)

- `UCX_<RenderMeshName>_NN` (01, 02 ...), each piece convex, closed, and in the same space as
  the render mesh (parent it to the render mesh, identity parent inverse). Other prefixes UE
  reads: `UBX_` box, `USP_` sphere, `UCP_` capsule.
- Few verts per hull (8 for boxes, under ~32-64 for hulls). Snap points to a coarse cell before
  hulling to reduce counts (recipe: hull_mesh_from_points). Several simple hulls beat one detailed one.
- Walls/floors: one box per piece. Doorways: 3 boxes around the opening (two jambs + lintel).
- Sockets: empties named `SOCKET_<Name>` parented to the mesh.

## 14. Geometry nodes and procedural stacks

- Build node groups in Python with the 4.0 interface API:
  `ng.interface.new_socket(name, in_out='INPUT'|'OUTPUT', socket_type='NodeSocketGeometry')`
  (the pre-4.0 `ng.inputs.new` is gone). Set a modifier input by the socket's `identifier`:
  `getattr(mod.properties.inputs, socket.identifier).value = value` on 5.x (where
  `mod[identifier] = value` raises `TypeError`), `mod[socket.identifier] = value` on 4.x; branch
  on `hasattr(mod, "properties")` (`set_gn_input` in the recipes).
- Everything not realized is not exported: end instancing chains with Realize Instances.
- Node and socket names change between versions; look them up with
  `[s.name for s in node.inputs]` before linking by name.
- Modifier stacks are evaluated on export (FBX `use_mesh_modifiers` default). Decide per asset
  whether to ship live modifiers or apply them; apply before UV/bake work.

## 15. Kitbashing

- Append from other .blend files (verified):
  ```python
  with bpy.data.libraries.load("//kits/parts.blend", link=False) as (src, dst):
      dst.objects = [n for n in src.objects if n.startswith("SM_Bolt")]
  for o in dst.objects:
      bpy.context.scene.collection.objects.link(o)
  ```
  Check the source's scale and units first; appended objects bring their materials along.
- Join by moving mesh data with bmesh or `bpy.ops.object.join` under a context override; then
  clean, re-UV, and unify materials (see `blender-materials-baking`).

## 16. Verify your work

1. `blender_info {file}`: dimensions in cm match the brief; no unapplied scale/rotation;
   triangle count within budget; UV maps present (UV0 textures, UV1 lightmap if used);
   materials named `M_`; UCX_ and SOCKET_ roles detected; LOD names correct.
2. `blender_render` views front, three_quarter, top with `color: "RANDOM"` (separate parts
   and stray pieces show up), and workbench default (cavity shows shading/normal problems).
   Render LODs side by side. Render modular pieces together to check joins.
3. `blender_mesh_check {file, objects}`: no problems; warnings judged (n-gons only where
   planar). Run `mesh_report` from the recipes as well for non-manifold edges: 0 on closed
   props.
4. Texel density within 10% of the project target.
5. Skinned meshes: `blender_rig_check` (it needs an armature in the file).
6. When the Unreal editor is up, `blender_to_unreal` and read its size and problem report.

Never report a mesh done without having looked at at least one render of it. Save to a new
file (`save_as`) unless the user asked you to modify the source.

## Pitfalls

- `matrix_world` is stale after setting transforms until `view_layer.update()`.
- `bmesh.ops.create_uvsphere(..., calc_uvs=True)` writes UVs only if a UV layer already exists
  in the bmesh (`bm.loops.layers.uv.new("UVMap")` first).
- `bmesh.ops.convex_hull` leaves the input faces in place; hull a fresh bmesh of points.
- UV operators need Edit Mode and an active, selected object; restore Object Mode afterwards.
- Scale on the object (not applied) scales bevel widths, texel density and collision in UE.
- Decimate destroys shape keys and is not for deforming meshes without checking weights.
- Booleans and Solidify can flip normals; always recalc and render.
