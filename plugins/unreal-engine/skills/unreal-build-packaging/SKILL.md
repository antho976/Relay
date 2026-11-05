---
name: unreal-build-packaging
description: Building and shipping Unreal Engine 5 projects - UBT targets and modules, build configurations, Build and RunUAT per OS, BuildCookRun, packaging settings, dedicated servers, headless CI, GenerateProjectFiles, compile, link and UHT errors, and plugin authoring. Use for any compile, build, package, cook, ship, CI or build-error task.
---

# Unreal Build and Packaging

Terminology: **build** = compile C++ with UBT; **cook** = convert assets to a platform's
runtime format; **stage** = gather binaries + cooked content into a folder; **package/pak**
= put content into `.pak` (and IoStore `.utoc/.ucas`) containers; **archive** = copy the
staged build to an output folder. UAT's `BuildCookRun` does all of them.

Read `reference/uat-commands.md` for full command lines and flag tables (BuildCookRun,
BuildPlugin, commandlets, BuildGraph). See `unreal-fundamentals` for module/target basics,
`unreal-cpp` for code, `unreal-testing-debugging` for running tests in CI.

## 1. Targets, modules, configurations

- A **target** (`Source/<Name>.Target.cs`) is an executable: `TargetType.Game`, `Editor`,
  `Client`, `Server`, `Program`. The usual pair is `MyGame.Target.cs` and `MyGameEditor.Target.cs`.
- A **module** (`Source/<Module>/<Module>.Build.cs`) is a compilation unit (DLL in editor
  builds, statically linked in monolithic game builds). Modules are listed in the
  `.uproject` `Modules` array and added to targets via `ExtraModuleNames`.
- Configurations:

| Config | Game code | Engine code | Use |
|---|---|---|---|
| Debug | unoptimized | unoptimized | Engine debugging; needs a source-built engine for editor targets |
| DebugGame | unoptimized | optimized | Debugging your game code (best default for debugging) |
| Development | optimized | optimized | Everyday editor and test builds; stats, console, logs |
| Test | optimized | optimized | Near-Shipping perf testing; keeps some profiling (stats, console optional) |
| Shipping | optimized | optimized | Release: no console, stats, `check()` or most logging |

  Editor targets only build Debug, DebugGame, Development. Test and Shipping are for
  Game/Client/Server targets. The launcher (installed) engine ships Development and Shipping
  engine binaries plus DebugGame for your modules; it cannot build Server targets.

Example targets (5.3+; `Latest` tracks the engine's current defaults):
```csharp
// Source/MyGame.Target.cs
using UnrealBuildTool;
public class MyGameTarget : TargetRules
{
    public MyGameTarget(TargetInfo Target) : base(Target)
    {
        Type = TargetType.Game;
        DefaultBuildSettings = BuildSettingsVersion.Latest;
        IncludeOrderVersion = EngineIncludeOrderVersion.Latest;
        ExtraModuleNames.Add("MyGame");
    }
}
// Source/MyGameServer.Target.cs  (dedicated server; source-built engine required)
public class MyGameServerTarget : TargetRules
{
    public MyGameServerTarget(TargetInfo Target) : base(Target)
    {
        Type = TargetType.Server;
        DefaultBuildSettings = BuildSettingsVersion.Latest;
        IncludeOrderVersion = EngineIncludeOrderVersion.Latest;
        ExtraModuleNames.Add("MyGame");
    }
}
```
Pinning a specific `BuildSettingsVersion.V4/V5` instead of `Latest` avoids surprise
warnings on engine upgrades; match whatever the project already uses.

Module rules:
```csharp
// Source/MyGame/MyGame.Build.cs
using UnrealBuildTool;
public class MyGame : ModuleRules
{
    public MyGame(ReadOnlyTargetRules Target) : base(Target)
    {
        PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;
        PublicDependencyModuleNames.AddRange(new string[] {
            "Core", "CoreUObject", "Engine", "InputCore", "EnhancedInput" });
        PrivateDependencyModuleNames.AddRange(new string[] { "Slate", "SlateCore", "UMG" });
        if (Target.bBuildEditor)
        {
            PrivateDependencyModuleNames.Add("UnrealEd"); // editor-only code, guarded by WITH_EDITOR
        }
    }
}
```
Public dependencies: types used in your **public headers**. Private: used only in `.cpp`
files or private headers. Never make a runtime module depend on `UnrealEd` unconditionally:
the game target will fail to build/package. Put editor code in a separate `Editor` module.

## 2. Building

Prefer the `ue_build` MCP tool: `{"target": "MyGameEditor", "configuration": "Development"}`
(defaults: `<Project>Editor`, host platform, Development). It returns the output tail with errors.
The editor must be closed while compiling editor modules, or the human uses **Live Coding**
(Ctrl+Alt+F11 in the editor). With Live Coding active, a UBT build fails with "Unable to build
while Live Coding is active"; ask the human to close the editor or compile with Live Coding.
Live Coding handles function bodies well; header/layout changes (new `UPROPERTY`, new
`UCLASS`, changed constructors) need an editor restart and a full build.

Engine script locations (`<Engine>` = the engine root containing `Engine/`):

| OS | Build | UAT | Project files |
|---|---|---|---|
| Windows | `<Engine>\Engine\Build\BatchFiles\Build.bat` | `<Engine>\Engine\Build\BatchFiles\RunUAT.bat` | `<Engine>\GenerateProjectFiles.bat` (source engine) |
| Linux | `<Engine>/Engine/Build/BatchFiles/Linux/Build.sh` | `<Engine>/Engine/Build/BatchFiles/RunUAT.sh` | `<Engine>/Engine/Build/BatchFiles/Linux/GenerateProjectFiles.sh` |
| Mac | `<Engine>/Engine/Build/BatchFiles/Mac/Build.sh` | `<Engine>/Engine/Build/BatchFiles/RunUAT.sh` | `<Engine>/Engine/Build/BatchFiles/Mac/GenerateProjectFiles.sh` |

Manual build:
```
Build.bat MyGameEditor Win64 Development -Project="D:\Proj\MyGame\MyGame.uproject" -WaitMutex
./Build.sh MyGameEditor Linux Development -Project="/home/me/MyGame/MyGame.uproject" -WaitMutex
```
Find the engine for a project: `EngineAssociation` in the `.uproject` (`ue_project_info`
reports it). A version string (`"5.4"`) means an installed launcher engine; a GUID means
a source build registered on that machine.

Useful UBT flags: `-DisableUnity` (catch missing includes hidden by unity builds),
`-NoHotReloadFromIDE`, `-Clean` (clean the target), `-verbose`.

## 3. Project files

IDE project files are generated, never committed (`*.sln`, `.vs/`, `Intermediate/ProjectFiles`).
- Windows: right-click the `.uproject` > **Generate Visual Studio project files**.
- Any OS, installed engine: `Build.bat -projectfiles -project="<abs>.uproject" -game -rocket -progress`
  (Linux/Mac: the platform `Build.sh` with the same arguments). Formats: `-vscode`,
  `-CMakefile`, `-Makefile` (the Mac script generates Xcode projects by default).
- Source engine: `GenerateProjectFiles.bat -project="<abs>.uproject" -game -engine`
  (`-engine` includes engine source in the solution).
- Rider opens the `.uproject` directly. Regenerate after adding/removing source files or modules.

## 4. Packaging

**Editor**: Platforms > (platform) > **Package Project** (choose the configuration in the
same menu). Advanced: **Tools > Project Launcher** (custom profiles for cook-by-the-book,
DLC, deploy-and-launch to devices; its layout differs between 5.x versions).

**Command line** (what CI runs):
```
RunUAT.bat BuildCookRun -project="D:\Proj\MyGame\MyGame.uproject" -noP4 ^
  -platform=Win64 -clientconfig=Shipping ^
  -build -cook -stage -pak -iostore -archive -archivedirectory="D:\Out" ^
  -prereqs -nodebuginfo -unattended -utf8output
```
Output: staged at `Saved/StagedBuilds/<CookPlatform>/` (e.g. `Windows`, `Linux`), archived
at `<archivedirectory>/<CookPlatform>/`. Add `-distribution` for store builds, `-compressed`
to compress pak content, `-map=/Game/Maps/Main+/Game/Maps/Menu` to limit cooked maps,
`-skipcook` / `-skipbuild` to reuse previous steps. Full flag table in the reference.

Packaging settings (Project Settings > Project > Packaging, saved in `Config/DefaultGame.ini`
under `[/Script/UnrealEd.ProjectPackagingSettings]`):
```ini
[/Script/UnrealEd.ProjectPackagingSettings]
BuildConfiguration=PPBC_Shipping
UsePakFile=True
bUseIoStore=True
bCompressed=True
ForDistribution=False
IncludePrerequisites=True
+MapsToCook=(FilePath="/Game/Maps/MainMenu")
+MapsToCook=(FilePath="/Game/Maps/Level01")
+DirectoriesToAlwaysCook=(Path="/Game/Data")
+DirectoriesToAlwaysStageAsUFS=(Path="Movies")
```
Change these through the settings UI when possible and review the ini diff; field names
vary slightly across versions.

**What gets cooked**: maps in *List of maps to include in a packaged build* (all project maps if the list is empty),
the startup/default maps (`[/Script/EngineSettings.GameMapsSettings]` `GameDefaultMap` in
`DefaultEngine.ini`), everything they hard- or soft-reference through `FSoftObjectPath`/
`TSoftObjectPtr` properties, *Additional Asset Directories to Cook*, and primary assets per
the Asset Manager. Assets loaded only by **string path built at runtime** are not
discovered and will be missing in the package ("Failed to load" in the packaged log).
Fix by referencing them from a property, a primary asset, or an always-cook directory.

**Asset Manager** (Project Settings > Game > Asset Manager, `DefaultGame.ini`
`[/Script/Engine.AssetManagerSettings]`): declare primary asset types to scan (e.g. your
`UPrimaryDataAsset` subclasses for items, levels, characters) with directories and cook
rules; load them with `UAssetManager::LoadPrimaryAsset`. Primary Asset Labels assign
assets to chunks for DLC/patching (`Generate Chunks` in Packaging).

**Pak / IoStore**: `-pak` puts content in `.pak`; `-iostore` (default for new projects)
adds `.utoc/.ucas` containers with faster loading. Keep both on for shipping.
Loose files (`-pak` off) are only for debugging.

## 5. Platform notes

- **Windows**: Visual Studio 2022 with the C++ game development workload and a Windows
  SDK (the engine's release notes list the exact versions). Prerequisites installer via `-prereqs`.
- **Linux**: native build needs the engine's bundled clang toolchain (`Setup.sh` fetches it for
  source builds). From Windows, install the cross-compile toolchain version matching the
  engine release and set `LINUX_MULTIARCH_ROOT`; then `-platform=Linux`. Dedicated servers
  are commonly built this way (`-serverplatform=Linux`).
- **Android**: Android Studio + SDK/NDK versions from the engine docs; run
  `Engine/Extras/Android/SetupAndroid.bat` (or `.sh`); Project Settings > Platforms > Android
  (package name, SDK license acceptance). Package with `-platform=Android -cookflavor=ASTC -package`.
- **Mac / iOS**: requires Xcode on a Mac (iOS can remote-build from Windows but signing still
  needs Apple tooling). Use `-platform=Mac` / `-platform=IOS` with `-package`.
- Consoles: under NDA; platform extensions provided by the platform holder.

## 6. Dedicated server

1. Source-built engine (from GitHub). 2. Add `MyGameServer.Target.cs` (above).
3. Guard client-only code (`UE_SERVER`, `IsRunningDedicatedServer()`, `#if !UE_SERVER`).
4. Package: `BuildCookRun ... -server -serverplatform=Linux -serverconfig=Development -noclient -build -cook -stage -pak -archive`.
5. Run: `MyGameServer.exe /Game/Maps/Arena -log` (Linux: `MyGameServer.sh`). See `unreal-multiplayer`.

## 7. CI

- Run everything headless: `-unattended -nullrhi -nosplash -nopause -stdout -FullStdOutLogOutput -utf8output`
  on editor/commandlet invocations; `-unattended` and `-noP4` (unless using Perforce) on
  UAT; `-buildmachine` marks a build-farm run (stricter, no interactive prompts).
- Cache compiled shaders and derived data. A shared DDC (network share or Unreal Cloud
  DDC) is configured in `DefaultEngine.ini` under the DDC backend graph, commonly with an
  environment override `UE-SharedDataCachePath=\\server\DDC`. 5.4+ projects use Zen-based
  local caching; check the engine's `BaseEngine.ini` `[DerivedDataBackendGraph]` for your
  version before editing. Pre-fill with `-run=DerivedDataCache -fill` (reference).
- Keep `Intermediate/`, `Binaries/`, `Saved/` and DDC out of git; cache them in CI instead.
- Typical pipeline: build editor target (for cooking) -> run automation tests ->
  BuildCookRun client/server -> archive artifacts + symbols (`.pdb`/`.debug`) separately.
- Fail the pipeline on `Error:` lines in the log and on non-zero exit codes; UAT returns
  non-zero on cook errors.
- The agent session cannot build GUI platforms it lacks SDKs for; say so plainly instead of
  claiming a build passed.

## 8. Common build errors and fixes

| Symptom | Cause | Fix |
|---|---|---|
| `Cannot open include file: 'X.h'` / `fatal error: 'X.h' file not found` | Module owning X not in dependencies, or wrong include path | Add the module to `Public/PrivateDependencyModuleNames`; include paths are relative to that module's `Public/` (or `Classes/`) folder |
| `LNK2019 unresolved external symbol` (MSVC) / `undefined reference to` (clang) | Missing module dependency; class/function in another module lacks `<MODULE>_API`; function declared but never defined | Add the dependency; add `MYMODULE_API` to the class/function; implement it. For engine classes without `ENGINE_API` on a method, you cannot call it from outside the module |
| `LNK2005 already defined` | Function defined in a header without `inline`, or included `.cpp` | Move to `.cpp` or mark `inline`/`FORCEINLINE` |
| `Unrecognized type 'FFoo' - type must be a UCLASS, USTRUCT, UENUM, or global delegate` (UHT) | Reflected property/param uses a non-reflected type, or the header defining it is not visible | Make it a `USTRUCT`/`UENUM`, include the header, or remove `UPROPERTY` |
| `#include found after .generated.h file` (UHT) | `*.generated.h` not the last include | Move it to the last include line |
| UHT error on a `UPROPERTY` whose type is a UObject held by value | UObjects are always referenced by pointer | Use `TObjectPtr<UFoo>` (or `TSubclassOf`, `TSoftObjectPtr`) |
| UHT error about an invalid class/struct prefix | Name prefix does not match the base type | `A` for Actors, `U` for other UObjects, `F` for structs, `E` for enums, `I` for interfaces |
| Circular include / incomplete type errors | Headers include each other | Forward declare (`class UFoo;`) in headers, include in `.cpp`; keep `TObjectPtr<UFoo>` members with forward declarations |
| `Circular dependency` between modules | Module A and B depend on each other | Extract shared types into a third module, or invert with interfaces/delegates |
| Builds locally, fails on CI with missing includes | Unity build hid the missing include | Build with `-DisableUnity` and add the include |
| `The following modules are missing or built with a different engine version` (dialog on editor start) | Binaries stale or built for another engine | Rebuild with `ue_build` (editor closed) or click Yes to rebuild |
| `Plugin 'X' failed to load because module 'X' could not be found` | Plugin binaries missing for this engine version | Build the project (plugin in `Plugins/` compiles with it) or `RunUAT BuildPlugin` (reference); marketplace plugins need the version for your engine |
| Renamed class/struct breaks Blueprints | Serialized references use the old name | Add a CoreRedirect in `DefaultEngine.ini`: `[CoreRedirects]` `+ClassRedirects=(OldName="/Script/MyGame.OldName",NewName="/Script/MyGame.NewName")` |
| Package fails with `LogCook: Error` / `Failed to load` | Missing/broken asset reference, editor-only asset referenced at runtime | Read the cook log in `Saved/Logs` or UAT log dir; fix the reference in the editor |
| `Unable to build while Live Coding is active` | Editor open with Live Coding | Close editor or use Live Coding |

Diagnosis order: read the **first** error (later ones cascade), identify the module, check
its `Build.cs`, then includes, then `_API` exports. For more detail read the full UBT log
(its path is printed at the end of the build output) or `ue_log` for editor-side errors.

## 9. Plugin authoring

Layout:
```
Plugins/MyTools/
  MyTools.uplugin
  Source/MyToolsRuntime/{MyToolsRuntime.Build.cs, Public/, Private/}
  Source/MyToolsEditor/{MyToolsEditor.Build.cs, Public/, Private/}
  Content/            (if CanContainContent)
  Resources/Icon128.png
```
```json
{
  "FileVersion": 3,
  "Version": 1,
  "VersionName": "1.0",
  "FriendlyName": "My Tools",
  "Description": "Runtime helpers and editor tools.",
  "Category": "Gameplay",
  "CreatedBy": "Studio",
  "CanContainContent": true,
  "EnabledByDefault": false,
  "Modules": [
    { "Name": "MyToolsRuntime", "Type": "Runtime", "LoadingPhase": "Default" },
    { "Name": "MyToolsEditor",  "Type": "Editor",  "LoadingPhase": "Default" }
  ],
  "Plugins": [
    { "Name": "EnhancedInput", "Enabled": true }
  ]
}
```
- Module `Type`: `Runtime` (everywhere), `Editor` (editor only), `UncookedOnly` (editor and
  uncooked, e.g. custom Blueprint nodes), `DeveloperTool`, `RuntimeNoCommandlet`, etc.
- `LoadingPhase`: `Default` usually; `PostConfigInit`/`PreDefault` for things the game
  module needs earlier; `PostEngineInit` for editor tooling that needs the engine up.
- Each module needs `IMPLEMENT_MODULE(FDefaultModuleImpl, MyToolsRuntime)` (or a custom
  `IModuleInterface` class) in one `.cpp`.
- Enable it in the `.uproject` `Plugins` array (`{"Name": "MyTools", "Enabled": true}`)
  and add the runtime module to dependent modules' `Build.cs`.
- Restrict platforms with `"PlatformAllowList": ["Win64", "Linux"]` on a module.
- Distribute compiled: `RunUAT BuildPlugin -Plugin="<abs>/MyTools.uplugin" -Package="<abs>/Out"`.

## The C++ loop on Linux

There is no Live Coding on Linux: every C++ change means close, build, relaunch (one to five
minutes). Do it in one call: `ue_build {"restart_editor": true}` saves and quits the editor,
waits for it to exit and for the Remote Control port to be released, builds, relaunches, and
waits until the editor answers. Batch C++ changes to pay that cost less often.

Two traps `ue_build` now guards against:
- A leftover **CrashReportClient** makes UnrealBuildTool think an editor is running, so it builds
  a numbered hot-reload module (`libUnrealEditor-MyGame-0003.so`) while
  `Binaries/Linux/UnrealEditor.modules` keeps pointing at the old one, and the editor runs old
  code. `ue_build` stops this project's leftover crash reporters first (another project's may
  be a dialog still open, so those are listed in a warning and left running) and warns when
  the manifest names a numbered module. If it does: quit the editor, delete the numbered files, build again.
- Building while the editor is open is refused unless `allow_editor_open` is set.

`ue_build` finds the engine without `UE_ROOT`: from a running editor, the project's last log
("Base Directory"), a source tree around the project, `Install.ini` (by GUID or by version), and
common install folders; the error lists what it tried.

## Verify your work

- [ ] `ue_build` succeeded for the editor target; for runtime-affecting changes also build
      the Game target (`{"target": "MyGame", "configuration": "Development"}`) so editor-only
      dependencies are caught.
- [ ] No new warnings in the changed modules; first error fixed first.
- [ ] Packaging changes: a BuildCookRun completed and the packaged log has no `Error:` lines
      and no `Failed to load` for your assets.
- [ ] `.uproject`, `.uplugin`, `*.Target.cs`, `*.Build.cs` and ini diffs are reviewed; no
      generated files (`*.sln`, `Intermediate/`, `Binaries/`) staged.
- [ ] If a platform could not be built in this session (missing SDK), the summary says so.
