---
name: blender-materials-baking
description: PBR materials and texture baking in Blender for Unreal - Principled BSDF inputs that map to Unreal, texture naming and ORM packing, sRGB versus Non-Color, shader nodes from Python, Cycles baking (normal, AO, procedural to texture), texel density, what FBX carries, slot naming. Use whenever you create or change materials, textures or bakes, or prepare a texture set for Unreal.
---

# Materials and baking for Unreal

Blender materials are a preview and a bake source. What reaches Unreal is: material slot
names and order, UVs, vertex colors, and the texture files you bake and save. Unreal
materials are rebuilt in Unreal (`unreal-materials-vfx`). Plan every material around the
texture set you will ship.

Verified code for everything below: [reference/bake-recipes.md](reference/bake-recipes.md).
Read it before writing bake or node-tree code; copy from it.

Related: `blender-modeling` (UVs, texel density, cleanup before baking), `blender-fundamentals`
(bpy, context, saving), `blender-to-unreal` (export/import); Unreal side
`unreal-materials-vfx`, `unreal-performance`.

## 1. Principled BSDF -> Unreal (4.0+ input names)

| Principled input (4.x name) | Unreal material pin | Texture / channel | Color space |
|---|---|---|---|
| Base Color | Base Color | `T_X_BC` RGB (A = opacity if needed) | sRGB |
| Metallic | Metallic | `T_X_ORM` B | Non-Color |
| Roughness | Roughness | `T_X_ORM` G | Non-Color |
| (no input) | Ambient Occlusion | `T_X_ORM` R | Non-Color |
| Normal (via Normal Map node, Tangent) | Normal | `T_X_N` | Non-Color |
| Emission Color x Emission Strength | Emissive Color (x scalar param) | `T_X_E` | sRGB |
| Alpha | Opacity (Translucent) / Opacity Mask (Masked) | `T_X_BC` A or `T_X_A` | Non-Color |
| Specular IOR Level (keep 0.5) | Specular (default 0.5) | usually none | - |
| Coat, Sheen, Subsurface, Transmission | shading models (Clear Coat, Cloth, Subsurface) / translucency | rebuild in UE | - |

- Renamed in 4.0: `Emission` -> `Emission Color`, `Specular` -> `Specular IOR Level`,
  `Subsurface` -> `Subsurface Weight`, `Clearcoat` -> `Coat Weight`, `Transmission` ->
  `Transmission Weight`. List inputs with `[i.name for i in node.inputs]` before linking by name.
- Values in the metalness workflow: metals Metallic 1 with Base Color 0.5-1.0 bright; non-metals
  Metallic 0, Base Color albedo in roughly sRGB 30-240 (no pure black/white). Metallic is
  binary except at transitions (dirt, edges).
- Opacity preview in Blender: 4.0/4.1 `material.blend_method` ('CLIP', 'HASHED', 'BLEND');
  4.2+ EEVEE Next `material.surface_render_method` ('DITHERED', 'BLENDED'). Guard with
  `hasattr`. This never reaches Unreal; set Blend Mode there.

## 2. Texture set naming and packing

- Names: `T_<Asset>_BC` (base color; `_D` in some projects - follow the project), `_N` (normal),
  `_ORM` (packed), `_E` (emissive), `_M` / `_Mask` (masks), `_H` (height), `_A` (opacity).
  One texture set per material. Match the Unreal team's naming if the project has one
  (check `unreal-fundamentals` / the project's conventions).
- ORM = Occlusion R, Roughness G, Metallic B. All Non-Color. In Unreal: sRGB off, Compression
  "Masks (no sRGB)".
- Normal: Unreal is DirectX (green down, -Y). Bake with `normal_g='NEG_Y'`, or bake OpenGL
  (Blender's default `POS_Y`) and tick "Flip Green Channel" on the Unreal texture - do exactly
  one of the two, and write down which. Unreal compression "Normalmap" (BC5), sRGB off.
- Base color and emissive: sRGB on. Everything else: Non-Color in Blender, sRGB off in Unreal.
- Power-of-two sizes (512, 1024, 2048, 4096). Non-square allowed (2048x1024), still powers of two.
- Pack only grayscale maps into one texture; never pack color. Keep channels independent
  (resolution of all three must match - the pack recipe asserts it).

## 3. Color space rules in Blender

- Image Texture nodes for data (N, ORM, masks, height, roughness): `image.colorspace_settings.name = 'Non-Color'`.
  Base color / emissive: `'sRGB'`.
- Bake targets: set the colorspace on the target image before baking. A value baked into an
  sRGB image is gamma-encoded; into Non-Color it is stored linear (0.7 in -> 0.703 out, verified).
- Save with `image.filepath_raw = path; image.file_format = 'PNG'; image.save()`. Float-buffer
  images save as 16-bit PNG. Do not use `image.save_render()` for data maps - it applies the
  view transform (AgX by default in 4.x) and alters values (verified).
- Generated images live only in memory: save or pack them before Blender exits
  (`blender_python` runs a fresh process each call - an unsaved bake is lost).

## 4. Building node trees from Python

- `mat.use_nodes = True`; nodes by `bl_idname`: `ShaderNodeBsdfPrincipled`,
  `ShaderNodeTexImage`, `ShaderNodeNormalMap`, `ShaderNodeSeparateColor` (3.3+; replaces
  Separate RGB), `ShaderNodeCombineColor`, `ShaderNodeMix` (3.4+ generic mix; `data_type='RGBA'`),
  `ShaderNodeVertexColor` (`layer_name`), `ShaderNodeAttribute`, `ShaderNodeBump`,
  `ShaderNodeTexNoise`, `ShaderNodeValToRGB` (color ramp), `ShaderNodeEmission`,
  `ShaderNodeOutputMaterial`.
- Link by socket name after checking names; multiple sockets can share a name (e.g. Mix
  node's A/B per data type) - index `node.inputs[i]` when names repeat.
- Recipe `build_pbr_material` builds BC + ORM (Separate Color G->Roughness, B->Metallic) + N.
- Assign: `obj.data.materials.append(mat)`; per-face slot via `polygon.material_index`.
  Slots live on the mesh data by default (`slot.link == 'DATA'`).

## 5. Baking with Cycles from Python

Setup (recipe "Cycles setup"): `scene.render.engine='CYCLES'`, `cycles.device='CPU'`,
`cycles.samples` 16-64 (default 4096 is far too slow), and `cycles.use_denoising = False`.
On builds without OpenImageDenoise (this one) a bake with denoising on finishes "successfully"
and writes nothing (verified). Background CPU baking works; EEVEE and Workbench cannot bake.

The bake contract:
1. Low poly has non-overlapping UVs in the active UV layer (unique UVs; for mirrored halves,
   offset one side by +1 in U before baking AO/normals, move it back after).
2. Every material on the low poly has an Image Texture node holding the target image,
   selected and active (`node_tree.nodes.active`), and not linked into the shader
   (recipe `set_bake_target` does all slots).
3. Selection: high polys selected, low poly selected and active (`select_for_bake`).
4. `bpy.ops.object.bake(type=..., use_selected_to_active=True, cage_extrusion=..., margin=...)`.
   Operator arguments override `scene.render.bake` settings for that call.
5. Save the image to a file; save the .blend if you want the bake nodes kept.

Types: `NORMAL`, `AO`, `ROUGHNESS`, `DIFFUSE` (with `pass_filter={'COLOR'}` = pure albedo),
`EMIT`, `GLOSSY`, `COMBINED`, `SHADOW`, `POSITION`, `UV`, `ENVIRONMENT`, `TRANSMISSION`.
No METALLIC: bake anything else by routing it to an Emission shader and baking `EMIT`
(recipe `bake_value_via_emit`).

High to low:
- `cage_extrusion` (metres): how far rays start outside the low poly. Start at ~1-2% of the
  object size, raise until no holes, lower if details from neighbouring surfaces leak in.
  `max_ray_distance` 0 = unlimited. A custom cage: `use_cage=True, cage_object="<name>"`.
- Low-poly shading must match what ships: triangulate and set normals/sharp edges before
  baking (Blender and Unreal both use MikkTSpace; triangulating first avoids differences in
  how quads are split). Split UVs on hard edges.
- Modifiers on the high poly are evaluated; no need to apply subdivision first.
- AO and COMBINED see every renderable object - the factory Cube around a small asset makes
  AO black (verified). Wrap the bake in `only_render([...])` or remove strays.
- Explode bakes: for multi-part assets whose parts occlude each other wrongly, move part
  pairs apart (same offset for high and low), bake, move back.
- Margin (padding): ~8 px at 1k, 16 at 2k, 32 at 4k; `margin_type='EXTEND'` or `'ADJACENT_FACES'`.

Procedural to texture (single object, `use_selected_to_active=False`): bake DIFFUSE/COLOR for
base color, ROUGHNESS, NORMAL (captures Bump/Normal Map inputs), EMIT-trick for metallic,
masks and height. With several selected objects each needs its own active image target
or the bake errors "No active image found".

Performance: CPU bake time scales with pixels x samples x high-poly density. Bake at 512
first to check, then at final size; raise `timeout_s` on `blender_python` (a 2k AO at 64
samples can take minutes). Bake one map per `blender_python` call if time is tight and save
each image immediately.

## 6. Texel density consistency

Hold every asset to the project density (e.g. 5.12 px/cm = 512 px/m). A texture's size is
chosen from the asset's UV area at that density, not from importance alone. Measure with
`texel_density` in `blender-modeling`'s recipes before baking; after changing UV scale
you must re-bake. Tiling materials (walls, floors) use world-sized UVs (1 UV unit = texture
world size, e.g. 2 m) and may exceed 0-1; trim sheets map strips of one texture.

## 7. What transfers through FBX (and what does not)

Verified by exporting and re-importing: material slot names and order, the UV layers (in
order: first layer = Unreal UV0), color attributes (as vertex colors), and simple hookups -
an Image Texture straight into Base Color, a Normal Map node into Normal - come back.
Everything else is dropped: ORM via Separate Color, procedural nodes, mixes, ramps, bump,
group nodes, blend modes. Therefore:
- Bake procedural work to textures before export.
- Rebuild the material in Unreal: one master material `M_<Family>` with parameters, and a
  Material Instance `MI_<Asset>` per texture set (see `unreal-materials-vfx`).
- Name each Blender material after the Unreal material that will be assigned to that slot,
  normally the instance: `MI_Crate_Wood`. Unreal names the mesh's material slot from it and,
  when it creates materials on import, the asset too. Avoid creating a pile of auto-generated
  materials: import without creating materials and assign the MIs, or let
  `blender_to_unreal`'s `materials` option handle it.
- Each slot is a mesh section and a draw call: 1-2 slots for props, more only when needed
  (different shading models, translucency). Remove unused slots before export.
- Textures are imported separately in Unreal (or by `blender_to_unreal` when it copies
  them); do not rely on FBX embedding (`embed_textures` / `path_mode='COPY'` are for other tools).

## 8. Vertex colors for masks

- Color attributes (3.2+ API): `mesh.color_attributes.new(name, type='BYTE_COLOR', domain='CORNER')`
  (or `'POINT'`); fill with `foreach_set("color", ...)` (RGBA floats). Set
  `color_attributes.active_color` and `render_color_index` to the one to export.
- The FBX exporter writes color attributes (`colors_type` 'SRGB' default, 'LINEAR', 'NONE').
  In Unreal read them with the Vertex Color node; typical use: R/G/B masks for blending
  dirt, moss, wetness, tint variations; A for wind or edge wear.
- Byte colors are sRGB-encoded in storage; if Unreal math needs linear masks, use values 0 or 1
  (which survive any curve) or check the import's vertex color handling.
- Bake into vertex colors: `scene.render.bake.target = 'VERTEX_COLORS'` (Cycles).

## 9. Verify your work

1. `blender_python`: run the recipe `audit_images()` - every image saved to a file, power of two,
   BC/E sRGB, N/ORM/M/H Non-Color, names `T_<Asset>_<Suffix>`.
2. Print pixel means of each bake (`stats`): a tangent normal map averages about (0.5, 0.5, >0.9);
   an AO map is mostly light; all-zero or all-0.5 gray means the bake did not run (check
   denoising, active image node, selection, UVs).
3. Look at the PNGs you wrote (read the image files) and at `blender_render` with
   `engine: "eevee"` or `"cycles"` and `color: "TEXTURE"` in workbench to see textures on the
   mesh. Seams, black islands, stretched texels and bake leaks show up there.
4. `blender_info`: material names/count per mesh, UV maps (UV0 first), images listed.
5. After import (`blender_to_unreal`), confirm the slot names and that textures in Unreal
   have the right sRGB and compression settings (see `unreal-materials-vfx`).

## Pitfalls

- Empty bake output: denoiser on without OIDN; image node not active; wrong UV layer active;
  image in the node is not the one you saved.
- Image node linked into the shader it bakes = circular dependency; keep the target unlinked.
- Overlapping UVs (mirrors, stacked islands) corrupt AO/normal bakes.
- Mixing OpenGL/DirectX normals: bumps look inverted under side light in Unreal.
- ORM with AO baked with other objects present, or with the ground plane, looks dirty.
- `bpy.ops.object.bake` works on selected objects; unrelated selected objects without a
  target image raise "No active image found".
- Bake settings on the scene persist into the saved file; that is harmless, but reset
  `render.engine` if the file's render setup matters to someone.
