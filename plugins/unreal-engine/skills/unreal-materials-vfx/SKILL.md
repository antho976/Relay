---
name: unreal-materials-vfx
description: Unreal Engine 5 materials, textures and Niagara - PBR, domains, blend and shading models, instances and parameters, functions, Substrate, texture settings, dynamic materials from code, decals, post process, Niagara systems and spawning, scalability, shader cost, Python material scripting. Use when creating or editing materials, shaders, textures, particles, VFX or decals, or cutting GPU cost.
---

# Materials and VFX

Materials and Niagara systems are binary assets: edit them through the editor via
`ue_python` (MaterialEditingLibrary, EditorAssetLibrary) or give the human click-paths.
C++ only drives them at runtime (parameters, spawning).

References:
- `reference/material-python.md` - building materials, functions and instances from Python;
  read before any scripted material work.
- `reference/niagara-cookbook.md` - spawning and controlling Niagara from C++/Blueprint,
  pooling, scalability, common effect recipes; read before any VFX work.

Related: `unreal-audio` (pair impacts with sound), `unreal-gas` (Gameplay Cues trigger VFX),
`unreal-multiplayer` (VFX are cosmetic: spawn via multicast/OnRep, never on dedicated servers),
`unreal-performance` (GPU profiling beyond materials and particles).

## PBR essentials

Metal/roughness workflow. Inputs of a Surface material:
- **Base Color** - albedo, no lighting baked in. Dielectrics roughly 50-240 sRGB; pure
  black/white are unrealistic.
- **Metallic** - 0 or 1 in almost all cases (texture masks for mixed surfaces), rarely in between.
- **Roughness** - the most important value for realism; drive it with a texture.
- **Specular** - leave at default 0.5 for nearly everything.
- **Normal** - tangent-space normal map (texture compression Normalmap, sampler type Normal).
- **Emissive** - HDR color; values > 1 bloom. With Lumen, emissive contributes indirect light.
- **Ambient Occlusion** - baked micro-occlusion only.
Pack grayscale maps into one texture (e.g. R=AO, G=Roughness, B=Metallic - "ORM"), set it to
compression `Masks` and **sRGB off**.

## Domain, blend mode, shading model

Material Domain (Details panel of the material):
- `Surface` - meshes (default). `Deferred Decal` - decals. `Light Function` - masks lights.
- `Post Process` - full-screen pass. `User Interface` - UMG. `Volume` - volumetric fog/clouds.

Blend Mode:
- `Opaque` - cheapest; use whenever possible.
- `Masked` - binary cutout via Opacity Mask (foliage, fences). Disables some early-Z
  optimizations; prefer geometry cutouts for large areas. Supported by Nanite (with cost).
- `Translucent` - glass, smoke, water surfaces; sorted, no depth write, overdraw-heavy,
  lighting mode matters (`Surface TranslucencyVolume` cheap, `Surface ForwardShading` for
  specular highlights). Not Nanite.
- `Additive` - glow/energy; `Modulate` - darkening multiply. `AlphaComposite` - premultiplied alpha.

Shading Model: `Default Lit`, `Unlit` (emissive only - UI-like, VFX, cheapest), `Subsurface`,
`Preintegrated Skin`, `Subsurface Profile` (skin), `Clear Coat` (car paint), `Two Sided Foliage`,
`Cloth`, `Eye`, `Hair`, `Single Layer Water`, `Thin Translucent`, `From Material Expression`.

### Substrate

Substrate replaces the fixed shading models with composable slabs (layered/multi-lobe
materials). It was Experimental from 5.2 and moved to Beta in later 5.x releases - check
the release notes for the project's engine. It is enabled per project (Project Settings >
Rendering > Substrate; requires restart and full shader recompile) and changes how every
material compiles. Legacy materials are converted automatically, but look and cost can change.
Do not enable it as part of another task; propose it to the human and test on a branch.

## Material instances and parameters

Rule: **one master material per surface family, many Material Instance Constants (MIC).**
MICs share the compiled shader of the parent (unless static switches differ), so they
compile nothing and load fast.

Parameter types (create with right-click in the graph or convert a constant with
"Convert to Parameter"):
- Scalar (`ScalarParameter`), Vector (`VectorParameter`), Texture (`TextureSampleParameter2D`,
  `TextureObjectParameter`), Static Switch (`StaticSwitchParameter`, compile-time branch -
  **each combination is a separate shader permutation**), Runtime Virtual Texture parameters.
- Group and Sort Priority parameters so instances stay usable.

Runtime changes:
- **Custom Primitive Data** (preferred for per-actor variation such as tint, damage flash):
  on the parameter node check "Use Custom Primitive Data" with an index, then in C++
  `MeshComp->SetCustomPrimitiveDataFloat(Index, Value)` (or `...Vector4`). No new material
  instance, batching and Nanite preserved.
- **Dynamic Material Instance (MID)** when you need textures or many parameters per actor:
  ```cpp
  UMaterialInstanceDynamic* MID = Mesh->CreateDynamicMaterialInstance(0); // slot 0; reuses existing MID on that slot
  MID->SetScalarParameterValue(TEXT("Glow"), 5.f);
  MID->SetVectorParameterValue(TEXT("Tint"), FLinearColor::Red);
  MID->SetTextureParameterValue(TEXT("Decal"), SomeTexture);
  ```
  Or `UMaterialInstanceDynamic::Create(ParentMaterial, this)` then `Mesh->SetMaterial(0, MID)`.
  Create once (BeginPlay) and cache in a `UPROPERTY()`; never create per Tick. Parameter
  names are case-insensitive `FName`s; a misspelled name fails silently.
- **Material Parameter Collection (MPC)** for global values (wind, time of day, player
  position for grass bending):
  ```cpp
  UKismetMaterialLibrary::SetScalarParameterValue(this, WeatherMPC, TEXT("Wetness"), 0.8f);
  ```
  (`#include "Kismet/KismetMaterialLibrary.h"`). Limits: 1024 scalar + 1024 vector
  parameters per collection; a single material can reference at most 2 MPCs. Adding or
  removing MPC parameters recompiles every material using it.

## Material functions and layers

- Material Function (`MaterialFunction` asset) = reusable subgraph with `FunctionInput` /
  `FunctionOutput` nodes; check "Expose to Library" to show it in the palette. Use for shared
  logic (triplanar, detail normal blending, wind). Changing it recompiles every user.
- Material Layers / Layer Blends: layered authoring in instances; heavier to compile, handy
  for artist-driven layering. Substrate projects use Substrate operators instead.

## Textures

Settings on each texture asset (Details > Compression / Texture / Level Of Detail):

| Texture | Compression | sRGB | Notes |
|---|---|---|---|
| Base color / albedo | Default (BC1/BC3; BC7 for quality) | on | |
| Normal map | Normalmap (BC5) | off | material sampler type Normal |
| ORM / masks | Masks | off | sampler type Linear Color / Masks |
| Single grayscale mask | Grayscale / Alpha | off | |
| HDR / emissive HDR | HDR (or HDR Compressed BC6H) | off | |
| UI | UserInterface2D | on | usually no mips (LOD group UI) |

- Dimensions: powers of two for mips and streaming (non-power-of-two textures get no mips
  and do not stream). Keep max size appropriate (use "Maximum Texture Size" or LOD Bias,
  not re-imports, to downscale).
- Mips: default `FromTextureGroup`; `NoMipmaps` only for UI and lookup textures.
- Texture Group (LOD group) controls streaming/filtering per category (World, Character, UI, ...).
- Streaming: the texture streamer loads mips by screen size. `stat Streaming` and the
  "Texture streaming pool over budget" message indicate problems; fix by reducing sizes, not by
  raising the pool blindly (`r.Streaming.PoolSize` is a per-platform tuning value).
- Virtual Texturing: Streaming Virtual Textures (very large textures, UDIMs) require
  Project Settings > Rendering > Virtual Textures enabled and the per-texture "Virtual Texture
  Streaming" flag; the material sampler must then be a virtual sampler type. Runtime Virtual
  Textures (RVT) cache material output (landscape blending, roads). Both are opt-in.
- Python: set `srgb`, `compression_settings`, `lod_group`, `mip_gen_settings` via
  `set_editor_property`; see `reference/material-python.md`.

## Decals

- Material: Domain `Deferred Decal`, Blend Mode `Translucent` (typical). Connected pins
  (Base Color, Normal, Roughness, Emissive, Opacity) define what the decal writes.
- Place a `DecalActor` (scale its box) or spawn at runtime:
  ```cpp
  UGameplayStatics::SpawnDecalAtLocation(this, DecalMaterial, FVector(8.f, 32.f, 32.f),
      Hit.ImpactPoint, Hit.ImpactNormal.Rotation(), /*LifeSpan*/ 10.f);
  ```
  (X of the size is the projection depth; the decal projects along its X axis.)
- Receivers need "Receives Decals" enabled (default on; turn off for characters/moving objects
  to avoid swimming). Fade with `UDecalComponent::SetFadeOut`. Budget the count; many large
  overlapping decals are expensive.

## Post-process materials

- Domain `Post Process`; read the scene with `SceneTexture` node (`PostProcessInput0` = scene
  color, `SceneDepth`, `CustomDepth`, `CustomStencil`); output to Emissive Color.
- Blendable Location (Details > Post Process Material): after tonemapping for stylization/UI
  overlays, before tonemapping for HDR-correct effects.
- Add to a Post Process Volume (Details > Rendering Features > Post Process Materials > +
  Asset reference; set "Infinite Extent (Unbound)" for global) or a camera's post-process settings.
- Outlines/highlights: Project Settings > Rendering > Custom Depth-Stencil Pass = "Enabled with
  Stencil"; on the mesh `SetRenderCustomDepth(true)` and `SetCustomDepthStencilValue(1)`;
  sample `CustomStencil` in the PP material.
- Every PP material is a full-screen pass - keep them cheap and few.

## Niagara overview

- **System** (`NiagaraSystem` asset, what you spawn) contains one or more **Emitters**; each
  emitter runs module stacks: Emitter Spawn/Update, Particle Spawn/Update, Event handlers,
  Render (sprite, mesh, ribbon, light, component renderers).
- **User Parameters** (`User.` namespace) are the public interface of a system - expose
  colors, counts, sizes, meshes, positions and set them from C++/BP.
- Sim target per emitter: **CPU** (default; supports events, collision via traces, lights,
  small counts) or **GPU Compute** (thousands+ of particles, depth-buffer collision, requires
  **fixed bounds**). Pick GPU above a few thousand particles.
- Lifecycle: Emitter State module (loop behavior, "Once" vs "Infinite", inactive response).
  One-shot systems should complete so pooled/auto-destroy components can be reclaimed.
- Scalability: an **Effect Type** asset per category (impacts, ambient, weapons) sets cull
  distances, max instances, and per-quality budgets; assign it on every system.
- Build.cs: add `"Niagara"`. Spawning code, pooling and parameter setting are in
  `reference/niagara-cookbook.md`.

Quick spawn:
```cpp
#include "NiagaraFunctionLibrary.h"
#include "NiagaraComponent.h"

UNiagaraComponent* FX = UNiagaraFunctionLibrary::SpawnSystemAtLocation(
    this, ImpactSystem, Hit.ImpactPoint, Hit.ImpactNormal.Rotation(),
    FVector(1.f), /*bAutoDestroy*/ true, /*bAutoActivate*/ true, ENCPoolMethod::AutoRelease);
if (FX) { FX->SetVariableLinearColor(TEXT("Color"), FLinearColor(1.f, 0.4f, 0.1f)); }
```
Hold system references as `UPROPERTY(EditDefaultsOnly) TObjectPtr<UNiagaraSystem>`, set in
Blueprint defaults - do not hard-code asset paths with `ConstructorHelpers` unless the
project already does.

Cascade (legacy particle system, `UParticleSystem`) is deprecated in UE5; do not create new
Cascade effects. Convert with the Cascade-to-Niagara converter plugin if needed.

## Performance

View modes (viewport > View Mode, or `ue_console` `viewmode <name>`):
- `Shader Complexity` (`viewmode shadercomplexity`) - green cheap, red/white expensive.
  Translucent stacks and large particles show up here.
- `Quad Overdraw` (`viewmode quadoverdraw`) - small triangles and overdraw.
- Lighting Only, Lightmap Density, Nanite visualizations (Nanite > Overview/Overdraw).
Stats: `stat gpu`, `stat unit`, `stat rhi`, `stat Niagara`, `profilegpu` (GPU frame capture in
the editor), Unreal Insights for deeper profiling.

Rules:
- Instruction count: Material editor > Window > Stats / "Platform Stats". Keep base pass
  surfaces lean; move constant math into material instances or vertex shader
  (Customized UVs / Vertex Interpolator node).
- Texture samples: pack channels; share samplers ("Shared: Wrap" sampler source) to stay under
  the 16-sampler limit.
- Permutations: every static switch combination and every "Usage" flag (Used with Skeletal
  Mesh, Niagara Sprites, Instanced Static Meshes, ...) compiles more shaders. Only enable
  usages needed; `Automatically Set Usage in Editor` adds them silently - review before shipping.
- Translucency and overdraw dominate VFX cost: fewer, larger-coverage-aware particles;
  use Opaque/Masked meshes for debris; limit screen-filling translucent sprites; use
  "Cutout" sub-UV/bounding geometry for sprites with lots of transparent pixels.
- Particle lights and per-particle collision are expensive; cap them.
- Prefer Custom Primitive Data over MIDs; prefer MICs over duplicating master materials.
- World Position Offset: keep simple; on Nanite meshes WPO has real cost - disable it at
  distance ("World Position Offset Disable Distance" on the primitive).

## Workflow: new material through MCP

1. Search for an existing master material first: `ue_search_assets` with
   `class_names: ["Material"]` and a query such as "Master" or "M_". Prefer creating an
   instance of an existing master.
2. Otherwise create it with `ue_python` (see `reference/material-python.md`), wrapped in
   `unreal.ScopedEditorTransaction`, then `recompile_material` and `save_asset`.
3. Create instances, set parameters, assign to meshes/actors, save.
4. Check the log (`ue_log` filter `Material|Shader|Error`) for compile errors.
5. Tell the human the asset paths created/changed and what they should look at in the
   viewport.

## Common pitfalls

- Normal map with sRGB on or compression Default - lighting looks wrong.
- Masks texture with sRGB on - values are gamma-curved.
- Creating a MID every frame or per hit - memory growth and hitches.
- Parameter name mismatch between C++ and material - no error, no effect.
- Static switches in instances used as "runtime toggles" - they cannot change at runtime
  and each combination is a new shader.
- GPU emitter without fixed bounds - particles disappear when the origin is off-screen.
- Spawning VFX on a dedicated server - wasted CPU; guard with net mode or use Gameplay Cues/multicast.
- Enabling Substrate or virtual texturing casually - project-wide shader recompile.

## First-person effects: prefer opaque

Translucent primitives close to a first-person camera (muzzle flashes, tracers, impact cards)
can white out the whole view when fired: they cover the screen at point-blank range and stack.
Use opaque or masked materials with emissive for first-person effects, keep translucent sprites
small and away from the near plane, and check a firing sequence with `ue_play` screenshots
before calling it done.

## Verify your work

- [ ] Material compiles (no errors in `ue_log`), assets saved, paths reported to the human.
- [ ] Texture settings (sRGB, compression) match the table above.
- [ ] Runtime parameters: names match exactly; MIDs cached; CPD used where possible.
- [ ] Niagara system has an Effect Type; GPU emitters have fixed bounds; one-shots complete.
- [ ] Shader Complexity / Quad Overdraw checked for new translucent or VFX content.
- [ ] `ue_build` passes after C++ changes (Build.cs has `Niagara` if used).
