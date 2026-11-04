---
name: unreal-level-environment
description: Level design and environment art in Unreal Engine 5 - blockout, player metrics, flow and pacing, modular kits, Landscape, foliage and PCG, World Partition, Data Layers and HLOD, Level Instances, lighting (Lumen, sky, fog, post process), Nanite and shadows, Megascans, collision and navmesh. Use for any level building, dressing, lighting or streaming task, and for scripting level edits in Python.
---

# Level Design and Environment in Unreal Engine

Maps (`.umap`) and their actors are binary, so never edit them as text. Every level change goes
through the editor, either with Python via the `unreal` MCP tools (`ue_python`, `ue_level_actors`,
`ue_console`, `ue_search_assets`) or as exact click-paths for the human. Wrap mutations in
`unreal.ScopedEditorTransaction`, save them, and tell the human which maps and assets changed.

Reference files. Read each one when its topic comes up:
- `reference/level-metrics.md`: read it before you block out a space, size doors, corridors, jumps, cover or stairs, or set up a modular grid.
- `reference/lighting-recipes.md`: read it before you light a level (outdoor day, golden hour, night, interior, cave), set exposure, or debug Lumen or shadow problems.
- `reference/python-level-scripting.md`: read it before you write any editor Python that spawns, moves, organizes or bulk-edits actors, or that touches Data Layers or World Partition.

Related skills: `unreal-game-design` (what the level must teach and test), `unreal-narrative`
(environmental storytelling beats, trigger volumes for story), `unreal-materials-vfx` (landscape
and master materials, decals), `unreal-gameplay-framework` (game mode, spawning, triggers),
`unreal-multiplayer` (replicated level actors), `unreal-audio` (ambient zones), `unreal-ai` (navigation
use), `unreal-performance` (profiling), `unreal-editor-automation` (general editor Python) and
`unreal-source-control` (committing OFPA files).

## 1. Workflow order

1. **Brief**: the level's purpose (which mechanic it teaches or tests, and which story beat it carries), its length, and its critical path.
2. **Paper map / bubble diagram**: spaces as bubbles, connections as lines, and the critical path marked with gates and landmarks.
3. **Blockout (greybox)**: correct metrics, no art. Playtest it here, because it is the cheapest time to move walls.
4. **Gameplay pass**: enemies, pickups, triggers, navmesh, checkpoints. Playtest again.
5. **Art pass**: modular kit or Megascans replace the greybox, then Landscape, foliage and PCG dressing.
6. **Lighting pass**: key light, sky, fog, exposure, then local lights for guidance.
7. **Optimization pass**: Nanite, HLOD, culling, streaming, collision cleanup, and profiling.

Do not start art before the blockout plays well. Keep blockout geometry in its own outliner folder or Data Layer, so it can be hidden or removed.

## 2. Blockout and greybox

- **Modeling Mode** (toolbar mode dropdown > Modeling): Box, Cylinder, Stairs and other
  primitives, Boolean, Extrude, PolyEdit, and **Cube Grid** (fast grid-aligned block modeling). Output
  is a Static Mesh asset (or a Dynamic Mesh Actor, depending on the tool's output type setting), so it can
  be reused and later swapped. This is the best choice for UE5 blockouts.
- **BSP brushes** (Place Actors panel > Geometry: Box, Cylinder, Stairs; additive or subtractive):
  quick to make, but they don't scale well and render slowly. If you keep a shape, convert it with
  *Brush Settings > Create Static Mesh*. Avoid BSP in shipped levels.
- **Cubes on a grid**: engine `/Engine/BasicShapes/Cube` (100 cm) scaled on a snapping grid.
  This is easy to script from Python (see `reference/python-level-scripting.md`).
- **Snapping**: work on 10/50/100 cm grids with 15 degree rotation snaps. Use the End key to drop an actor to the floor.
- **Materials**: one greybox master material with a world-aligned grid texture (1 m cells), plus color
  coding (for example orange for climbable, red for hazard, blue for water). Keep the color language
  consistent, and write it into the level doc.

## 3. Metrics (1 Unreal unit = 1 cm)

Build from the *actual* character values, not guesses. Read them from the character asset:

```python
import unreal
cdo = unreal.get_default_object(unreal.load_class(None, "/Game/Characters/BP_Player.BP_Player_C"))
cap = cdo.get_editor_property("capsule_component")
mov = cdo.get_editor_property("character_movement")
print("capsule r/hh:", cap.get_unscaled_capsule_radius(), cap.get_unscaled_capsule_half_height())
for p in ["max_walk_speed", "jump_z_velocity", "gravity_scale", "max_step_height", "walkable_floor_angle", "air_control"]:
    print(p, mov.get_editor_property(p))
```

Jump height is `v^2 / (2 * g)`, where `g = 980 * GravityScale` (the default world gravity Z is -980).
Derive the jump distance, safe gap widths and ledge heights from it. Tables and defaults are in
`reference/level-metrics.md`.

## 4. Flow, landmarks, sightlines, pacing, gating

- **Critical path first**: the player should always be able to read the next goal. Use light, color,
  contrast, composition, and moving elements.
- **Landmarks**: one large, unique silhouette per region, visible from most of it (towers, mountains).
  Players orient by landmarks, not by the minimap.
- **Sightlines**: reveal the goal before the path to it (show, then make them work for it). Frame
  reveals through doorways. In shooters, control engagement distances by breaking long sightlines with cover.
- **Pacing**: alternate tension and release. Combat arena, then corridor or quiet space, then reward, then the next arena.
  Put the save and checkpoint at release points.
- **Gating**: locks and keys (literal keys, abilities, story flags, or Data Layer activation). Gates
  should be visible before they can be opened, which invites a return.
- **Loops over dead ends**: when an optional branch ends, give the reward and a shortcut back to the main path.
- **Affordance language**: climbable, breakable and interactive surfaces share consistent visuals across the whole game.
- **Pinch points**: use them before loading or streaming seams and set pieces.

## 5. Modular kits

- Design every piece on one grid (for example 100 cm or 400 cm walls, 300–400 cm floor-to-floor heights).
- **Pivots**: at a corner at floor level (or bottom center for props), never at the mesh center.
  Pieces should snap on grid increments with 90 degree rotations.
- Keep thickness consistent, and put walls on the grid line or offset consistently to one side. Hide seams with trim or pillars.
- Keep kit meshes Nanite-enabled where appropriate (§9), and use a shared master material with material
  instances. Vertex color or Custom Primitive Data gives variation without more materials.
- Name them `SM_<Kit>_<Piece>_<Size>` (for example `SM_Bunker_Wall_400x300`).
- Group reusable arrangements (a room corner, a window bay) as Level Instances or Packed Level Actors (§8).

## 6. Landscape

- Create it with the mode dropdown > Landscape > Manage > New. Use a recommended size from this table,
  which is sections per component times quads per section times components, plus 1:

| Overall size (vertices) | Quads/section | Sections/component | Components |
|---|---|---|---|
| 505 x 505 | 63 | 2x2 | 4x4 |
| 1009 x 1009 | 63 | 2x2 | 8x8 |
| 2017 x 2017 | 63 | 2x2 | 16x16 |
| 4033 x 4033 | 63 | 2x2 | 32x32 |
| 8129 x 8129 | 127 | 2x2 | 32x32 |

  At the default scale of 100, one vertex is 1 m, so 1009 x 1009 is about 1 km square. The Z scale
  of 100 gives about ±256 m of height. Fewer, larger components means fewer draw calls but coarser LOD and streaming.
- **Sculpt**: Sculpt, Smooth, Flatten, Ramp (roads), Erosion, and Noise. Import heightmaps as 16-bit grayscale PNG or RAW.
  Keep *Edit Layers* enabled (the default in UE5), so that non-destructive layers (base, roads,
  splines) can be toggled.
- **Paint layers**: in the landscape material, use a *Landscape Layer Blend* node (or Layer Sample
  nodes) whose layer names match the paint layers. Then in Landscape Mode > Paint, create a Layer
  Info (weight-blended) for each layer. Keep it to about 4–8 layers per component, because each extra
  layer per component costs texture samples. Use *Landscape Layer Coords* or world-aligned UVs and
  macro variation to break up tiling.
- **Grass**: a Landscape Grass Type asset plus a *Landscape Grass Output* node in the landscape
  material spawns dense, non-interactive grass per layer automatically. Use the Foliage tool
  or PCG for trees and interactive props instead.
- **Holes**: a Visibility layer with a Landscape Visibility Mask in the material (and masked
  blend mode on the hole material), for cave entrances.
- **Landscape splines** for roads and rivers deform the terrain and can place meshes.
- **Nanite Landscape** (recent 5.x; check its status in your version): Landscape details panel > Nanite >
  Enable Nanite. It can help with VSM and performance on high-end targets. Test it on the target hardware.

## 7. Foliage and PCG

**Foliage Mode** (mode dropdown > Foliage): drag meshes in to create Foliage Types (density, scale range, align to
normal, ground slope limits, cull distance, collision). Paint by hand for hero areas. Set cull
distances, and disable collision on small plants. For mass scattering, use the
**Procedural Foliage Spawner** plus a Procedural Foliage Volume. That may need enabling in
Editor Preferences > Experimental, so check your version. PCG supersedes it for new work.

**PCG framework** (plugin *Procedural Content Generation Framework*, `"PCG"` in the `.uproject`).
It was experimental in 5.2 and 5.3 and Beta in 5.4, and Epic's release notes list the core framework as
production-ready from 5.5. Check the release notes for your exact engine version. Sub-features such as
GPU execution and biome tooling have their own (often experimental) status.
- The assets are a PCG Graph and a PCG Volume (or a PCG Component on any actor, often with a spline).
- A typical scatter graph: *Get Landscape Data* (or *Surface Sampler* on the input) → *Density Filter* /
  attribute filters (slope, height) → *Difference* (subtract roads and buildings, using exclusion volumes or
  tagged actors) → *Transform Points* (random rotation and scale) → *Self Pruning* → *Static Mesh Spawner*.
- Splines: *Get Spline Data* → *Spline Sampler* for fences, rows of trees, or road edges.
- Output is instanced static meshes owned by the PCG component. Regenerate from the component's
  *Generate* button. Enable *Is Partitioned* on large worlds (World Partition), so generation happens per grid cell.
- Keep hand-placed hero assets out of PCG output. Use PCG for density, and people for composition.
- Generation modes: generate in the editor and save the result (cheap at runtime), or *Generate at
  Runtime* (5.4+) for large or dynamic worlds, which costs CPU at runtime.

## 8. World Partition, Data Layers, HLOD, OFPA, Level Instances

- **World Partition** is enabled for maps created from the Open World template, or converted through
  *Tools > Convert Level* (a commandlet under the hood; back up the map first). The map is one
  persistent level whose actors stream by a runtime grid: *World Settings > World Partition
  Setup > Runtime Settings* (grid cell size, loading range). Check *Enable Streaming*.
- **One File Per Actor (OFPA)**: each actor is saved in its own file under
  `Content/__ExternalActors__/` (and `__ExternalObjects__/`). This makes collaboration and source control
  per actor, so the human must commit those folders. After a scripted change, several small `.uasset` files change, not the `.umap`.
- **Editor loading**: the World Partition window (Window > World Partition > World Partition Editor)
  loads regions for editing. Unloaded actors are not visible to Python `get_all_level_actors()`.
- **Is Spatially Loaded**: disable it on actors that must always be present (game logic managers).
  Keep it on for scenery.
- **Data Layers**: Data Layer Assets plus Data Layer Instances in the world's Data Layers Outliner
  (Window > World Partition > Data Layers Outliner). *Editor* data layers organize work, while *Runtime* data
  layers are loaded or activated at runtime, for example for story states (village intact vs. burned) or
  events. Switch them at runtime through the Data Layer Manager (5.3+):

```cpp
#include "WorldPartition/DataLayer/DataLayerManager.h"
if (UDataLayerManager* DLM = UDataLayerManager::GetDataLayerManager(this))
{
	DLM->SetDataLayerRuntimeState(VillageBurnedLayer /*UDataLayerAsset* UPROPERTY*/, EDataLayerRuntimeState::Activated);
}
```
  (Before 5.3 this lived on `UDataLayerSubsystem`. Check your version's headers.)
- **HLOD**: create HLOD Layer assets (Instancing, Merged Mesh or Simplified Mesh approximation), assign
  them in World Settings (the default HLOD layer) or per actor, then use *Build > Build HLODs*. Rebuild
  after large changes, because stale HLODs show old geometry at a distance.
- **Level Instance**: select actors, right-click > Level > *Create Level Instance*. It becomes a reusable
  sub-level asset placed like an actor, and edited in context. A **Packed Level Actor** (right-click > Level >
  *Create Packed Level Actor*) bakes static meshes into instanced static meshes for rendering
  efficiency. Use it for repeated static arrangements with no logic.
- Non-World-Partition projects use **level streaming** (Levels window, sub-levels,
  `Load Stream Level`) instead. Don't mix the two approaches in one map.

## 9. Lighting, Nanite and Virtual Shadow Maps (summary)

Defaults for new UE5 projects: Lumen for global illumination and reflections, Virtual Shadow Maps, and Nanite enabled.
The settings are in Project Settings > Engine > Rendering: *Dynamic Global Illumination Method*, *Reflection
Method*, *Shadow Map Method*, *Generate Mesh Distance Fields* (required for software Lumen), and
*Support Hardware Ray Tracing* / *Use Hardware Ray Tracing when available*. These are stored in
`Config/DefaultEngine.ini` under `[/Script/Engine.RendererSettings]`, which you can read as text.

- **Outdoor sky stack**: Directional Light (*Atmosphere Sun Light* on) + Sky Atmosphere + Sky Light
  (*Real Time Capture* on) + Volumetric Cloud (optional) + Exponential Height Fog (Volumetric Fog optional).
  *Window > Env. Light Mixer* creates any missing pieces.
- **Exposure**: one unbound Post Process Volume per level. Set Auto Exposure Min and Max EV100 ranges to suit
  the scene, rather than leaving them wide open, and use Exposure Compensation for tuning. The values per scene
  type are in `reference/lighting-recipes.md`.
- **Lumen gotchas**: thin walls (under about 10 cm) leak light, so use thick walls or a hidden blocker. Emissive
  surfaces are not a replacement for real lights. Large meshes get poor Mesh Distance Fields and Lumen cards,
  so split them. Use *Show > Visualize > Lumen Scene* and *Mesh Distance Fields* to debug.
- **Nanite**: enable it for high-poly static geometry, kit pieces, rocks and Megascans.
  *Not* for translucent materials (unsupported), for tiny meshes with few triangles where the overhead
  dominates, for skeletal meshes (treat animated characters as non-Nanite; Nanite skinning is only
  experimental in the newest versions), or when the target platform doesn't support Nanite (mobile, older GPUs). Nanite with masked materials or
  World Position Offset (foliage wind) works in 5.1+, but it costs more. Profile it. Use the viewport *Nanite
  Visualization* modes (Overview, Overdraw) to inspect.
- **Virtual Shadow Maps**: pair them with Nanite. Non-Nanite meshes with World Position Offset
  invalidate shadow cache pages every frame, so limit WPO distance on foliage. Watch
  `stat gpu` and the *Virtual Shadow Map* visualization.
- Full recipes and troubleshooting: `reference/lighting-recipes.md`.

## 10. Fab / Megascans import

- **Fab** is the current source. The Fab editor plugin (bundled with newer engines, or installed from the
  launcher/Fab for older 5.x; opened from the Window menu or a Content Browser button, depending on
  version) imports Megascans and marketplace assets directly into the project. Quixel Bridge was the
  earlier path. Epic has moved Megascans to Fab, so confirm licensing on the listing.
- After import: move assets to a consistent folder (`/Game/Environment/Megascans/...`), check scale
  (Megascans are real-world scale in cm), enable Nanite on high-poly meshes, and route the imported materials
  through the project's master material if it has one (see `unreal-materials-vfx`).
- Check texture sizes. 8K textures on small props waste memory, so set Max Texture Size or LOD bias.

## 11. Collision

- **Simple collision** (boxes, spheres, capsules, convex hulls) is used for movement and physics. **Complex** (per-poly)
  collision is for precise traces only. Set *Collision Complexity* per static mesh, and avoid *Use Complex As
  Simple* on anything simulated or on large, dense meshes.
- Author collision in the DCC tool with `UCX_<MeshName>_##` (convex), `UBX_` (box), `USP_` (sphere) and `UCP_` (capsule)
  prefixed meshes, or generate it in the Static Mesh Editor (*Collision* menu), or with Python
  (`unreal.StaticMeshEditorSubsystem.add_simple_collisions`, see the scripting reference).
- **Collision presets**: `BlockAll` (walls), `NoCollision` (grass, small decor),
  `BlockAllDynamic`, `OverlapAllDynamic` (triggers). Invisible blockers keep players in bounds.
  Put a Blocking Volume at level edges and on complex ledges. It is smoother than complex mesh collision.
- Visualize collision with the viewport *Show > Collision* flag (`show collision` in the console), or with the *View Mode > Collision > Player Collision* view mode.

## 12. Navigation

- Add a **Nav Mesh Bounds Volume** covering the playable area, and press **P** in the viewport to
  show the navmesh. A `RecastNavMesh` actor appears, holding the agent settings (Agent Radius, Agent
  Height, Agent Max Slope, Agent Max Step Height, Cell Size). Project-wide agents are under Project
  Settings > Engine > Navigation System > Supported Agents.
- **Runtime Generation** (Project Settings > Navigation Mesh, or the RecastNavMesh actor): *Static* for fixed levels
  (cheapest), *Dynamic Modifiers Only* when only nav modifiers or obstacles change, *Dynamic* when
  geometry moves (costs CPU).
- **Nav Modifier Volume** with a Nav Area class (for example `NavArea_Null` to block, or custom costs). A **Nav Link Proxy**
  handles jumps and drops. Set *Can Ever Affect Navigation* off on small clutter to keep the navmesh clean.
- For World Partition, navmesh has World Partition-specific settings (for example
  *Is World Partitioned* on the RecastNavMesh). Check your version's docs before enabling them on big worlds.
- Verify it by playing AI through the level, or with `ue_console` `show Navigation`.

## 13. Scripting level work (the agent's main path)

Use `ue_level_actors` to inspect, and `ue_python` to mutate. Core pattern:

```python
import unreal
eas = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
mesh = unreal.EditorAssetLibrary.load_asset("/Engine/BasicShapes/Cube")
with unreal.ScopedEditorTransaction("Blockout: arena walls"):
    for i in range(8):
        a = eas.spawn_actor_from_object(mesh, unreal.Vector(i * 400.0, 0.0, 150.0),
                                        unreal.Rotator(roll=0.0, pitch=0.0, yaw=0.0))
        a.set_actor_scale3d(unreal.Vector(4.0, 0.2, 3.0))   # 400 x 20 x 300 cm
        a.set_actor_label(f"BO_Wall_{i:02d}")
        a.set_folder_path("Blockout/Arena")
unreal.get_editor_subsystem(unreal.LevelEditorSubsystem).save_current_level()
```

Placing along splines, bulk mobility changes, folder organization, Data Layers, Nanite toggling,
collision generation, lighting setup and saving under OFPA are all in `reference/python-level-scripting.md`.

## 14. Common pitfalls

- `unreal.Rotator(a, b, c)` positional order is **roll, pitch, yaw**. Always use keywords.
- Scripted edits in unloaded World Partition regions: those actors aren't in memory, so load the region first.
- Forgetting that OFPA saves actors to `__ExternalActors__`, then telling the human only the `.umap` changed.
- Movable mobility on everything. Static meshes that never move should be *Static*, because it is cheaper and gives better cached shadows.
- Leaving blockout BSP and the greybox material in shipped maps.
- Light leaking through thin Lumen geometry, and exposure pumping from wide auto-exposure ranges.
- Complex collision on the player's walkable surfaces, which causes snagging. Missing collision on small ledges, where players get stuck.
- Navmesh not rebuilt, or bounds not covering new areas. Press P to check.

## Relay tools for this work

- `ue_asset_audit {path}` before a lighting or streaming investigation: textures that cannot
  stream (not power of two, Never Stream), wrong compression, meshes without collision, dense
  meshes without Nanite or LODs, missing references and redirectors. The World Partition texture
  problems that look like engine bugs are usually one of these.
- `ue_screenshot` and `ue_play` to see the level from the player's height.

## 15. Verify your work

- [ ] `ue_level_actors` shows the expected actors, labels and folders, and there are no stray actors at the origin.
- [ ] Metrics were checked against the real character values: doors, gaps and steps traversable in PIE.
- [ ] Lighting: no black or blown-out areas at the target exposure. `stat unit` and `stat gpu` are within budget on the target hardware.
- [ ] Navmesh (P) covers the playable areas. AI can path across the level.
- [ ] Collision is checked in Player Collision view, and there are no invisible snags on the critical path.
- [ ] HLODs are rebuilt if World Partition content changed. PCG is regenerated and saved.
- [ ] Changes were saved (`save_current_level` / `save_dirty_packages`). The human was told which maps, external actor files and assets changed.
- [ ] `ue_log` with filter `Error|Warning` shows nothing new (look for map check errors, lighting or navmesh warnings).
