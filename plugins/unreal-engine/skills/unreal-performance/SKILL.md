---
name: unreal-performance
description: Profiling and optimizing Unreal Engine 5 games - frame budgets, stat commands, Unreal Insights, ProfileGPU, tick management, GC and pooling, async loading, memory and texture streaming, scalability and device profiles, Lumen, Nanite and shadow costs, draw calls and instancing, LODs. Use when a game is slow, hitches, stutters or runs out of memory, or before shipping.
---

# Unreal Performance

Measure, find the bound, fix the biggest cost, measure again. Never optimize from a guess.
Console commands referenced here are listed in `reference/console-commands.md` - read it
when you need an exact command or cvar; do not invent cvars.

## 1. Budgets

| Target | Frame budget |
|---|---|
| 30 FPS | 33.3 ms |
| 60 FPS | 16.6 ms |
| 90 FPS (VR) | 11.1 ms |
| 120 FPS | 8.3 ms |

Game thread, render thread (Draw) and GPU run in parallel, pipelined across frames. The
frame time is roughly the **maximum** of them, not the sum. Aim for ~10-15% headroom below
the budget on the target hardware. Budget sub-systems explicitly when the project is
serious (e.g. at 60 FPS: gameplay 4 ms, animation 2 ms, physics 1.5 ms, UI 1 ms on the game
thread; lighting/shadows/post budgets on GPU) and write them in the project docs.

## 2. Measure correctly

- Profile a **packaged Development or Test build**, or at least Standalone Game
  (`UnrealEditor <Project>.uproject -game`), on target-class hardware. Editor viewport numbers
  include editor overhead; Shipping strips stats and most trace scopes.
- Disable caps while measuring: `t.MaxFPS 0`, `r.VSync 0`. Fix the resolution
  (`r.ScreenPercentage 100` or a known upscaler setting) so runs are comparable.
- Use a repeatable scenario: a fixed camera path (Sequencer), a saved game, or a
  `-ExecCmds` script. Record the same scene before and after every change.
- Warm up: the first seconds include shader compilation (PSO) and streaming hitches. Look
  at steady state separately from hitches.

With the editor live you can run commands through `ue_console`, e.g.
`{"command": "stat unit"}`; results are on screen for the human, while logged output
(`stat dumpframe`-style dumps, `memreport`) goes to `ue_log` or `Saved/Profiling`.

## 3. Find the bound

`stat unit` shows **Frame**, **Game** (game thread), **Draw** (render thread), **RHIT**
(RHI thread, when present), **GPU** and memory. `stat unitgraph` plots them over time.

| Largest | Bound | Next step |
|---|---|---|
| Game | CPU game thread | `stat game`, Insights CPU track, tick audit (section 5) |
| Draw / RHIT | Render thread (draw calls, visibility) | `stat scenerendering`, `stat initviews`, `stat rhi`, instancing (section 8) |
| GPU | GPU | `stat gpu`, `ProfileGPU`, view modes (section 7) |
| Frame high but all low | Waiting / vsync / frame cap / hitches | check `t.MaxFPS`, `r.VSync`, `stat unitgraph` spikes |

Quick GPU-bound test: `r.ScreenPercentage 50`. If frame time drops a lot, you are
pixel/GPU bound (post, lighting, overdraw); if not, look at the CPU or at vertex/geometry cost.

## 4. Unreal Insights (the main profiler)

Traces capture CPU scopes on every thread, GPU passes, frames, bookmarks, logs, loading and memory.

- Launch with tracing: `<Game or UnrealEditor> <Project>.uproject -game -trace=default`
  (default = cpu, gpu, frame, log, bookmark, and more). Add channels:
  `-trace=default,memory` (memory allocations; must be enabled at startup),
  `-trace=default,loadtime,file` (loading), `-trace=default,task` (task graph),
  `-statnamedevents` (turns stat scopes into named trace events; adds overhead).
- Write to a file instead of a live Insights session: `-tracefile=<path>.utrace` (5.x).
- At runtime (5.x): `Trace.Start default`, `Trace.Stop`, `Trace.Bookmark <name>`;
  `Trace.File` / `Trace.Send <host>` in versions that have them. Check with `help` in the
  console if unsure.
- Open `Engine/Binaries/<Platform>/UnrealInsights(.exe)`; live sessions and stored traces
  appear in the session browser. Timing Insights: frames, per-thread timelines, the
  Timers table (inclusive/exclusive time per scope), the Counters panel. Memory Insights:
  allocations by tag and callstack. Loading Insights: package load timings.

Add your own scopes in C++ so Insights shows game code by name:
```cpp
#include "ProfilingDebugging/CpuProfilerTrace.h"

void UInventoryComponent::RebuildCache()
{
    TRACE_CPUPROFILER_EVENT_SCOPE(UInventoryComponent::RebuildCache);
    for (const FItem& Item : Items)
    {
        TRACE_CPUPROFILER_EVENT_SCOPE_STR("Inventory.ItemPass");
        // ...
    }
}
```
For `stat` groups (shown by `stat MyGame`):
```cpp
DECLARE_STATS_GROUP(TEXT("MyGame"), STATGROUP_MyGame, STATCAT_Advanced);
DECLARE_CYCLE_STAT(TEXT("AI Perception Update"), STAT_MyGame_Perception, STATGROUP_MyGame);

void UMyPerception::Update()
{
    SCOPE_CYCLE_COUNTER(STAT_MyGame_Perception);
    // ...
}
```
Use `QUICK_SCOPE_CYCLE_COUNTER(STAT_MyGame_Thing)` for a one-off scope. Mark phases with
`TRACE_BOOKMARK(TEXT("BossFightStart"))` so they are easy to find in the timeline.

## 5. CPU: game thread

Ticks are the most common cost. Audit them:
- `dumpticks` prints every registered tick function (and whether it is enabled) to the log.
- Default to **no tick**. Turn it on only when something must run every frame:
```cpp
AMyPickup::AMyPickup()
{
    PrimaryActorTick.bCanEverTick = false;          // no tick function at all
}

AMyTurret::AMyTurret()
{
    PrimaryActorTick.bCanEverTick = true;
    PrimaryActorTick.bStartWithTickEnabled = false; // enable when a target is in range
    PrimaryActorTick.TickInterval = 0.1f;           // 10 Hz is enough for aiming logic
    PrimaryActorTick.TickGroup = TG_PrePhysics;
}
// later: SetActorTickEnabled(true); SetActorTickInterval(0.2f);
```
  Components use `PrimaryComponentTick` the same way. Blueprint actors: Class Defaults >
  Actor Tick > Start with Tick Enabled off, or Tick Interval.
- Replace polling with events: timers (`GetWorldTimerManager().SetTimer`), delegates,
  overlap events, gameplay messages. A timer at 0.25 s beats a tick that checks a condition.
- Tick groups: `TG_PrePhysics` (default; input/movement), `TG_DuringPhysics`,
  `TG_PostPhysics` (needs final physics transforms), `TG_PostUpdateWork` (cameras, late UI).
  Use `AddTickPrerequisiteActor/Component` for ordering instead of hacks.
- Blueprint cost: heavy loops, `Get All Actors Of Class` every frame, string building and
  per-frame casting in Blueprint graphs. Move hot paths to C++ (`unreal-cpp`, `unreal-blueprints`).
- Animation: `USkeletalMeshComponent::VisibilityBasedAnimTickOption =
  EVisibilityBasedAnimTickOption::OnlyTickPoseWhenRendered` for off-screen characters;
  `bEnableUpdateRateOptimizations = true` (URO) for distant ones; keep AnimBP logic in the
  thread-safe update path. `stat anim` shows the cost.
- Physics and movement: `CharacterMovementComponent` is expensive per character; reduce
  counts, simplify collision, disable `bGenerateOverlapEvents` where not needed.

**Significance manager** (plugin `SignificanceManager`, module `SignificanceManager`) scales
work by importance (distance, visibility): register objects with a significance function,
call `USignificanceManager::Update` from your game code each frame with the view
transforms, and react in the post-significance callback (change tick interval, disable
effects, lower anim rate). Check `SignificanceManager.h` in the engine for the exact
callback signatures of your version before writing the code.

**Garbage collection**: hitches in `stat unitgraph` at regular intervals and
`CollectGarbage` in Insights. Reduce UObject churn (pool instead of spawn/destroy, use
structs for data), avoid huge `TArray<UObject*>` graphs. `gc.TimeBetweenPurgingPendingKillObjects`
controls how often GC runs (default about 60 s). Incremental reachability analysis exists
as an opt-in in 5.4+ (`gc.AllowIncrementalReachability`); treat it as experimental.
Correctness first: every UObject pointer you keep must be a `UPROPERTY` (`TObjectPtr`) or
`TWeakObjectPtr`, otherwise GC frees it underneath you.

**Object pooling** for frequent spawn/destroy (projectiles, hit effects, damage numbers):
```cpp
AProjectile* UProjectilePool::Acquire(const FTransform& Xf)
{
    AProjectile* P = Free.Num() > 0 ? Free.Pop() : GetWorld()->SpawnActor<AProjectile>(ProjectileClass, Xf);
    P->SetActorTransform(Xf);
    P->SetActorHiddenInGame(false);
    P->SetActorEnableCollision(true);
    P->SetActorTickEnabled(true);
    P->Activate();   // your own reset function
    return P;
}
void UProjectilePool::Release(AProjectile* P)
{
    P->SetActorHiddenInGame(true);
    P->SetActorEnableCollision(false);
    P->SetActorTickEnabled(false);
    Free.Push(P);    // UPROPERTY() TArray<TObjectPtr<AProjectile>> Free;
}
```
Niagara has built-in pooling (`ENCPoolMethod` on `SpawnSystemAtLocation`).

## 6. Loading and memory

- **Hard references load everything they point to.** A Blueprint that hard-references
  every weapon mesh loads all of them with the Blueprint. Use `TSoftObjectPtr` /
  `TSoftClassPtr` for content that is optional or chosen at runtime, then load async:
```cpp
UPROPERTY(EditDefaultsOnly) TSoftObjectPtr<UStaticMesh> HeavyMesh;

void AMyActor::LoadHeavy()
{
    FStreamableManager& SM = UAssetManager::GetStreamableManager();
    Handle = SM.RequestAsyncLoad(HeavyMesh.ToSoftObjectPath(),
        FStreamableDelegate::CreateUObject(this, &AMyActor::OnHeavyLoaded));
}
void AMyActor::OnHeavyLoaded() { MeshComp->SetStaticMesh(HeavyMesh.Get()); }
```
  (`TSharedPtr<FStreamableHandle> Handle;` keeps it loaded; module deps `Engine`.) Avoid
  `LoadSynchronous()` during gameplay: it blocks the game thread. Primary assets and
  `UAssetManager::LoadPrimaryAsset` handle bundles of content (see `unreal-build-packaging`).
- Inspect references: Content Browser > right-click asset > **Size Map** (shows total size
  pulled in) and **Reference Viewer**.
- `memreport -full` writes a report to `Saved/Profiling/MemReports/`; read it with the
  file tools. It lists memory by class (`obj list`), textures, render targets, pools.
- `stat memory`, `stat streaming`; `listtextures` lists loaded textures and sizes.
- Texture streaming: if the log or screen shows the streaming pool over budget, textures
  go blurry. Fix the content (smaller textures, correct LOD groups, no 4K on small props,
  `Never Stream` only for UI) before raising `r.Streaming.PoolSize` (MB). Set pool size per
  platform in device profiles, not globally.
- Audio, animation and meshes also dominate memory; check `memreport` sections rather than assuming textures.

## 7. GPU

- `stat gpu` gives per-pass GPU times; `ProfileGPU` (Ctrl+Shift+Comma) captures one frame
  into the GPU Visualizer with a hierarchical pass breakdown.
- View modes (viewport **View Mode** menu, or `viewmode <name>`): `shadercomplexity`,
  `quadoverdraw`, `lightcomplexity`, `lightmapdensity`, `lodcoloration`, `wireframe`,
  `unlit`. Nanite (Nanite Visualization: Overview, Triangles, Clusters, Overdraw) and Lumen
  (Lumen Overview, Surface Cache, ...) visualizations are under the viewport View Mode menu.
- Typical GPU costs and fixes:
  - **Translucency / overdraw** (particles, foliage cards, glass): fewer layers, smaller
    particles, masked instead of translucent, `quadoverdraw` to find it.
  - **Shader complexity**: long materials on large screen areas; use material quality
    switches and static switches; see `unreal-materials-vfx`.
  - **Shadows**: Virtual Shadow Maps cost scales with invalidations (moving objects,
    WPO foliage) and resolution. Disable shadow casting on small/dynamic props, limit
    shadow-casting movable lights, tune `r.Shadow.Virtual.ResolutionLodBiasDirectional`.
  - **Lumen**: the biggest single GPU item in most UE5 scenes. Knobs: GI/Reflection quality
    via `sg.GlobalIlluminationQuality` / `sg.ReflectionQuality`; Post Process Volume Lumen
    settings (Final Gather Quality, Scene Detail, reflection quality); hardware ray tracing
    only when the target GPUs benefit. For low-end targets consider disabling Lumen in
    a scalability level and using baked lighting or SSGI. Keep walls thick enough for
    software Lumen (surface cache) to avoid light leaks.
  - **Nanite**: great for dense opaque geometry and cuts draw calls; costs are
    overdraw of many overlapping instances, masked/WPO materials (5.x supports them but
    they are slower), and very small instances. Not for translucent materials. Aggregate
    geometry (foliage) needs care; check the Nanite overdraw view.
  - **Resolution**: use an upscaler (TSR is the default AA in UE5) with `r.ScreenPercentage`
    below 100 on lower settings.

## 8. Draw calls and scene complexity (render thread)

- `stat scenerendering` shows mesh draw calls; `stat rhi` shows draw primitive calls and
  triangles; `stat initviews` shows visibility/culling cost.
- Many copies of the same mesh: use **Instanced Static Mesh** or
  **Hierarchical Instanced Static Mesh** components (Foliage uses HISM), or Nanite meshes,
  which batch automatically.
```cpp
// In the constructor
Instances = CreateDefaultSubobject<UInstancedStaticMeshComponent>(TEXT("Instances"));
// At runtime / construction
for (const FTransform& T : Transforms) { Instances->AddInstance(T); } // local space by default
```
- Merge static clutter: **Tools > Merge Actors** (or Packed Level Actors for reusable
  groups). Fewer, larger actors also reduce game-thread and streaming overhead.
- Culling: set **Max Draw Distance / Desired Max Draw Distance** on small props, use
  Cull Distance Volumes, keep precomputed visibility for static indoor scenes.
- Material slots: each section on a non-Nanite mesh is a draw call; reduce slot counts.

## 9. LODs and HLOD

- Non-Nanite static meshes: generate LODs in the Static Mesh editor (LOD Settings > Number
  of LODs / LOD Group), check with `viewmode lodcoloration`. Skeletal meshes need LODs
  regardless of Nanite (5.5+ has experimental Nanite skinned meshes; do not rely on it).
- `r.StaticMeshLODDistanceScale`, `r.SkeletalMeshLODBias`, `r.ForceLOD` are testing tools
  or scalability knobs, not content fixes.
- World Partition HLOD: define HLOD Layers, assign actors, then build HLODs (editor:
  **Build > Build HLODs**, or commandlet `-run=WorldPartitionBuilderCommandlet
  -Builder=WorldPartitionHLODsBuilder`; see `unreal-build-packaging` reference). HLODs replace
  distant unloaded cells with merged/simplified proxies and are essential for large worlds
  (`unreal-level-environment`).

## 10. Scalability and platforms

- Groups `sg.ViewDistanceQuality`, `sg.AntiAliasingQuality`, `sg.ShadowQuality`,
  `sg.GlobalIlluminationQuality`, `sg.ReflectionQuality`, `sg.PostProcessQuality`,
  `sg.TextureQuality`, `sg.EffectsQuality`, `sg.FoliageQuality`, `sg.ShadingQuality`
  (0 low - 3 epic, 4 cinematic). `scalability 0..3` sets all; `scalability auto` benchmarks.
- What each level means lives in `Config/DefaultScalability.ini` (overrides the engine's
  `BaseScalability.ini`):
```ini
[ShadowQuality@1]
r.Shadow.Virtual.ResolutionLodBiasDirectional=1.5
r.Shadow.MaxResolution=1024

[FoliageQuality@0]
foliage.DensityScale=0.4
```
  Copy a section from the engine's `Engine/Config/BaseScalability.ini` and edit it rather
  than writing a new one from memory; unspecified cvars fall back to the engine's values.
- Per-platform/per-device settings: `Config/DefaultDeviceProfiles.ini`
```ini
[Windows DeviceProfile]
+CVars=r.Streaming.PoolSize=2000
```
- Expose settings to players with `UGameUserSettings` (`SetOverallScalabilityLevel`,
  `ApplySettings`); see `unreal-ui-umg` for the settings menu.

## 11. The optimization loop

1. Define the budget and the target hardware; write them down.
2. Build a repeatable capture (packaged Development/Test build, fixed scenario).
3. Capture a baseline: `stat unit` numbers plus an Insights trace (and `ProfileGPU` if GPU bound).
4. Identify the bound and the single largest item in that bound.
5. Form one hypothesis, make one change (code, content via `unreal-editor-automation`, or
   config), rebuild.
6. Re-capture the same scenario; compare against the baseline. Keep the change only if
   the numbers moved; revert otherwise.
7. Record the result (commit message or perf log) and repeat from step 4.

Pitfalls:
- Measuring in the editor with other viewports, the Content Browser, or Live Coding active.
- Optimizing averages when players feel hitches: look at worst frames and `stat unitgraph` spikes.
- Changing several things at once: you will not know which helped.
- Turning features off globally (`r.Lumen...`, `r.Nanite 0`) instead of per scalability
  level: this hides the problem on high-end and breaks the look.
- Leaving debug cvars in `DefaultEngine.ini` `[SystemSettings]`. Review ini diffs.

## Relay tools for this work

- `ue_profile {seconds, warmup, play}` starts Play In Editor, records Unreal's CSV profiler and
  returns average, 95th percentile and worst frame, game thread, render thread and GPU times and
  the slowest frames. Measure before and after every optimisation, with the same map and camera.
- `ue_asset_audit` finds oversized or non-streaming textures and dense meshes without Nanite or
  LODs, which are the usual memory and streaming culprits.
- `ue_console` for `stat` commands; `ue_play` screenshots show hitches in context.

## Verify your work

- [ ] Before/after numbers from the same scenario and build type are recorded.
- [ ] The bound (Game/Draw/GPU) was identified before changing anything.
- [ ] Changes compile (`ue_build`) and `ue_log` shows no new warnings (e.g. streaming pool over budget).
- [ ] Every cvar written to an ini exists (checked in `reference/console-commands.md`, the
      console's autocomplete, or `help` in the console).
- [ ] Scalability/device-profile changes were tested at each affected quality level.
- [ ] Visual quality regressions (Lumen, shadows, LOD pops) were checked by the human.
