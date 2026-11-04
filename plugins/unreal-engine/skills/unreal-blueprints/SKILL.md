---
name: unreal-blueprints
description: Blueprints in Unreal Engine 5 - the C++/Blueprint split and exposing C++, Blueprint types, Data Assets, Data Tables and Curve Tables, creating and editing Blueprint assets through Python editor scripting, handing graph wiring to the human, Blueprint performance. Use for any Blueprint .uasset, data asset or data table, or deciding what goes in C++ versus Blueprint.
---

# Blueprints

Blueprints are binary assets. You cannot read or write their graphs as text. What you can do:
write the C++ they build on, create Blueprint assets and set their defaults/components through
Python in the live editor (`ue_python`), compile and save them, and give the human exact
instructions for graph wiring. Design every feature so that as little as possible depends on
graph wiring you cannot do.

Prerequisites: `unreal-fundamentals` (paths, CDO, hard/soft references), `unreal-cpp`
(specifiers; see its `reference/specifiers.md`). Gameplay classes: `unreal-gameplay-framework`.
General editor scripting recipes (assets, levels, MCP tool usage): `unreal-editor-automation`.
Widget Blueprints: `unreal-ui-umg`. Anim Blueprints: `unreal-animation`.

## The C++ / Blueprint split

Default architecture for agent-built features:
1. **C++ base class** holds state (UPROPERTYs), logic, and events. Everything testable and
   diffable lives here.
2. **Blueprint subclass** (`BP_...`) holds asset references (meshes, sounds, VFX, widget classes),
   tuned numbers, and optional cosmetic overrides. Ideally it is a *data-only Blueprint*:
   no graph logic, only defaults, which you can fully set from Python.
3. Gameplay code references the C++ class, never the Blueprint class, and spawns via a
   `TSubclassOf<>` property that the human (or your script) points at the Blueprint.

Put in C++: core rules, math, loops, anything run every frame, networking, save data, systems
used by many assets. Put in Blueprint: asset choices, per-asset tuning, one-off level scripting,
UI layout and animation, quick designer iteration.

### Exposing C++ to Blueprint

```cpp
UCLASS(Blueprintable, Abstract)
class MYGAME_API AInteractableBase : public AActor
{
    GENERATED_BODY()
public:
    // Called by Blueprint (exec node)
    UFUNCTION(BlueprintCallable, Category = "Interaction")
    void SetEnabled(bool bNewEnabled);

    // Getter node without exec pins
    UFUNCTION(BlueprintPure, Category = "Interaction")
    bool IsEnabled() const { return bEnabled; }

protected:
    // No C++ body; a Blueprint subclass implements it as an event. C++ calls OnInteracted(...).
    UFUNCTION(BlueprintImplementableEvent, Category = "Interaction")
    void OnInteracted(AActor* InteractingActor);

    // C++ default in CanInteract_Implementation; Blueprint may override and call Parent.
    UFUNCTION(BlueprintNativeEvent, BlueprintCallable, Category = "Interaction")
    bool CanInteract(AActor* InteractingActor) const;
    virtual bool CanInteract_Implementation(AActor* InteractingActor) const;

    UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Interaction")
    bool bEnabled = true;

    UPROPERTY(EditDefaultsOnly, BlueprintReadOnly, Category = "Interaction")
    TObjectPtr<USoundBase> InteractSound;
};
```
- `BlueprintImplementableEvent` returning `void` appears as a red Event node; with a return value
  or output parameters it appears as an overridable function (My Blueprint > Functions >
  Override).
- Call native events from C++ by their plain name (`CanInteract(Actor)`), never
  `CanInteract_Implementation` from outside the class.
- Static helpers go in a `UBlueprintFunctionLibrary`:
```cpp
UCLASS()
class MYGAME_API UMyGameStatics : public UBlueprintFunctionLibrary
{
    GENERATED_BODY()
public:
    UFUNCTION(BlueprintCallable, Category = "MyGame", meta = (WorldContext = "WorldContextObject"))
    static AActor* FindNearestInteractable(const UObject* WorldContextObject, FVector Location, float Radius);
};
```
- Events from C++ to Blueprint: `DECLARE_DYNAMIC_MULTICAST_DELEGATE_*` +
  `UPROPERTY(BlueprintAssignable)` (see `unreal-cpp` `reference/delegates-and-timers.md`).
- After changing reflected C++, full rebuild with the editor closed, then reopen; existing
  Blueprints pick up new members. Removing or renaming a member used in graphs breaks nodes: use
  `meta = (DeprecatedFunction)` / keep the old name, or add a Core Redirect in `DefaultEngine.ini`:
```ini
[CoreRedirects]
+PropertyRedirects=(OldName="/Script/MyGame.InteractableBase.bIsOn",NewName="/Script/MyGame.InteractableBase.bEnabled")
+FunctionRedirects=(OldName="/Script/MyGame.InteractableBase.Toggle",NewName="/Script/MyGame.InteractableBase.SetEnabled")
+ClassRedirects=(OldName="/Script/MyGame.OldDoor",NewName="/Script/MyGame.Door")
```

## Blueprint kinds

| Kind | Parent / factory | Use for | Scriptable from Python |
|---|---|---|---|
| Blueprint Class (Actor, Pawn, Character, Component, Object) | `BlueprintFactory` + `parent_class` | Subclassing C++ with assets and defaults | Create, defaults, components, compile |
| Data-only Blueprint | Same, with no graph logic | Pure configuration subclass | Fully |
| Blueprint Function Library | Human creates (Blueprint > Blueprint Function Library) | Static helpers in Blueprint | Prefer a C++ `UBlueprintFunctionLibrary` |
| Blueprint Macro Library | Human creates (Content Browser > Blueprint > Blueprint Macro Library) | Reusable macro graphs | No |
| Blueprint Interface | Human creates (Blueprint > Blueprint Interface) | Cross-class messaging | Prefer a C++ `UINTERFACE(Blueprintable)` |
| Widget Blueprint | `WidgetBlueprintFactory` (UMG editor) | UI; parent a C++ `UUserWidget` with `BindWidget` members | Create; layout editing is human work |
| Animation Blueprint | `AnimBlueprintFactory` + `target_skeleton` | Skeletal animation logic | Create; AnimGraph is human work |
| Level Blueprint | Per map | One-off level scripting only | No; avoid for reusable logic |
| Editor Utility Blueprint / Widget | Editor Utilities menu | Editor tooling for the human | Create only |

## Data: Data Assets, Data Tables, Curve Tables

| Use | When |
|---|---|
| `UDataAsset` subclass (`DA_`) | One object's worth of structured config (a weapon definition, a level's settings). Can hold object references, arrays, nested structs; per-asset diffs are separate files |
| `UPrimaryDataAsset` subclass | Same, plus Asset Manager discovery/async loading by `FPrimaryAssetId` (item catalogs, levels, characters) |
| `UDataTable` (`DT_`) with an `FTableRowBase` struct | Many homogeneous rows (spreadsheet-like balance data), importable from CSV/JSON; you can author rows as text and import |
| `UCurveTable` (`CT_`) / `UCurveFloat` | Values over a key (level-scaling curves) |

Data Table row struct:
```cpp
#include "Engine/DataTable.h"

USTRUCT(BlueprintType)
struct FWeaponRow : public FTableRowBase
{
    GENERATED_BODY()

    UPROPERTY(EditAnywhere, BlueprintReadOnly) float Damage = 10.f;
    UPROPERTY(EditAnywhere, BlueprintReadOnly) float FireRate = 0.2f;
    UPROPERTY(EditAnywhere, BlueprintReadOnly) TSoftObjectPtr<USkeletalMesh> Mesh;
};

// lookup
if (const FWeaponRow* Row = WeaponTable->FindRow<FWeaponRow>(RowName, TEXT("WeaponLookup"))) { ... }
```

Primary Data Asset:
```cpp
UCLASS(BlueprintType)
class MYGAME_API UWeaponData : public UPrimaryDataAsset
{
    GENERATED_BODY()
public:
    UPROPERTY(EditDefaultsOnly, BlueprintReadOnly, Category = "Weapon") FText DisplayName;
    UPROPERTY(EditDefaultsOnly, BlueprintReadOnly, Category = "Weapon") float Damage = 10.f;
    UPROPERTY(EditDefaultsOnly, BlueprintReadOnly, Category = "Weapon") TSoftObjectPtr<USkeletalMesh> Mesh;

    virtual FPrimaryAssetId GetPrimaryAssetId() const override
    {
        return FPrimaryAssetId(TEXT("Weapon"), GetFName());
    }
};
```
Register the type so the Asset Manager scans it (`DefaultGame.ini`):
```ini
[/Script/Engine.AssetManagerSettings]
+PrimaryAssetTypesToScan=(PrimaryAssetType="Weapon",AssetBaseClass=/Script/MyGame.WeaponData,bHasBlueprintClasses=False,bIsEditorOnly=False,Directories=((Path="/Game/Data/Weapons")),SpecificAssets=,Rules=(Priority=-1,ChunkId=-1,bApplyRecursively=True,CookRule=Unknown))
```
Editor path for the human: Project Settings > Game > Asset Manager > Primary Asset Types to Scan.

## Python editor scripting workflow

All snippets run through `ue_python` with the editor open. First call `ue_setup_check`; Python
needs `PythonScriptPlugin` (and `EditorScriptingUtilities` for the asset libraries). Before using
any class or function not shown here, check it exists: `print(hasattr(unreal, "X"))`,
`help(unreal.X)`, `print(dir(unreal.X))`. Names differ slightly between versions.

### Create a Blueprint subclass of a C++ class

```python
import unreal

path, name = "/Game/MyGame/Interactables", "BP_Door"
full = f"{path}/{name}"
if unreal.EditorAssetLibrary.does_asset_exist(full):
    bp = unreal.EditorAssetLibrary.load_asset(full)
else:
    factory = unreal.BlueprintFactory()
    factory.set_editor_property("parent_class", unreal.load_class(None, "/Script/MyGame.Door"))  # or unreal.Door
    bp = unreal.AssetToolsHelpers.get_asset_tools().create_asset(name, path, unreal.Blueprint, factory)
print(bp.get_path_name())
```
Parent can also be a Blueprint class: `unreal.EditorAssetLibrary.load_blueprint_class("/Game/.../BP_Base")`.

### Set class defaults (CDO)

```python
import unreal

with unreal.ScopedEditorTransaction("Set BP_Door defaults"):
    gen_class = unreal.EditorAssetLibrary.load_blueprint_class("/Game/MyGame/Interactables/BP_Door")
    cdo = unreal.get_default_object(gen_class)
    cdo.set_editor_property("enabled", True)         # C++ bEnabled -> Python enabled (bools drop the b); verify with dir(cdo)
    cdo.set_editor_property("interact_sound", unreal.load_asset("/Game/Audio/SC_DoorOpen"))
    # C++-defined component: edit through the CDO's component
    mesh = cdo.get_editor_property("door_mesh")      # UPROPERTY TObjectPtr<UStaticMeshComponent> DoorMesh
    mesh.set_editor_property("static_mesh", unreal.load_asset("/Game/Env/SM_Door"))
unreal.EditorAssetLibrary.save_asset("/Game/MyGame/Interactables/BP_Door", only_if_is_dirty=False)
```
- Python property names are snake_case of the C++ name; bools keep the `b` (`bEnabled` becomes
  `b_enabled`). Run `print([p for p in dir(cdo) if not p.startswith("_")])` when unsure.
- `set_editor_property` fails for properties not editable in the Details panel (e.g. only
  `BlueprintReadOnly` without `Edit*`). Make it `EditDefaultsOnly` in C++ if the script must set it.
- Changing Blueprint *member variables* (created in the Blueprint, not in C++) works the same way
  on the CDO once they exist and are compiled.

### Add components to a Blueprint (5.1+ SubobjectDataSubsystem)

```python
import unreal

bp = unreal.load_asset("/Game/MyGame/Interactables/BP_Door")
sds = unreal.get_engine_subsystem(unreal.SubobjectDataSubsystem)
handles = sds.k2_gather_subobject_data_for_blueprint(bp)
root = handles[0]                                    # usually the root/scene; inspect to be sure
params = unreal.AddNewSubobjectParams(parent_handle=root,
                                      new_class=unreal.PointLightComponent,
                                      blueprint_context=bp)
new_handle, fail_reason = sds.add_new_subobject(params)
if str(fail_reason):
    print("failed:", fail_reason)
sds.rename_subobject(new_handle, unreal.Text("DoorLight"))
data = unreal.SubobjectDataBlueprintFunctionLibrary.get_data(new_handle)
light = unreal.SubobjectDataBlueprintFunctionLibrary.get_object(data)
light.set_editor_property("intensity", 2000.0)
```
Verify each name with `help(unreal.SubobjectDataSubsystem)` first; argument names and return
shapes have varied across 5.x. If a component is always needed, prefer creating it in the C++
constructor with `CreateDefaultSubobject` instead.

### Compile and save

```python
import unreal

bp = unreal.load_asset("/Game/MyGame/Interactables/BP_Door")
if hasattr(unreal, "BlueprintEditorLibrary"):
    unreal.BlueprintEditorLibrary.compile_blueprint(bp)
else:
    print("BlueprintEditorLibrary not available; ask the human to press Compile")
unreal.EditorAssetLibrary.save_asset(bp.get_path_name().split(".")[0], only_if_is_dirty=False)
```
Then check `ue_log` with `filter: "Blueprint|Kismet|Error|Warning"` for compile errors.
`unreal.BlueprintEditorLibrary` also offers (verify with `dir()`): `reparent_blueprint`,
`refresh_all_nodes`, `remove_unused_variables`, `remove_unused_nodes`, `find_event_graph`,
`find_graph`, `rename_graph`, `replace_variable_references`, `get_blueprint_asset`,
`generated_class`. It does **not** let you create and wire arbitrary graph nodes.

### Create data assets and data tables

```python
import unreal
tools = unreal.AssetToolsHelpers.get_asset_tools()

# Data Asset instance of a C++ UDataAsset/UPrimaryDataAsset subclass
f = unreal.DataAssetFactory()
f.set_editor_property("data_asset_class", unreal.WeaponData)          # verify class exposed: hasattr(unreal, "WeaponData")
da = tools.create_asset("DA_Rifle", "/Game/Data/Weapons", unreal.WeaponData, f)
da.set_editor_property("damage", 25.0)

# Data Table with a C++ row struct, rows from CSV text you author
f = unreal.DataTableFactory()
f.set_editor_property("struct", unreal.WeaponRow.static_struct())
dt = tools.create_asset("DT_Weapons", "/Game/Data", unreal.DataTable, f)
csv = "Name,Damage,FireRate,Mesh\nRifle,25,0.1,\nShotgun,80,0.9,\n"
unreal.DataTableFunctionLibrary.fill_data_table_from_csv_string(dt, csv)
unreal.EditorAssetLibrary.save_loaded_assets([da, dt], only_if_is_dirty=False)
```
CSV first column is the row name; headers are property names as in C++. Keeping the CSV/JSON
source file in the repo (e.g. `Data/Weapons.csv`) and importing it makes balance data diffable.
`fill_data_table_from_json_string` / `fill_data_table_from_csv_file` / `fill_data_table_from_json_file`
also exist (verify with `dir(unreal.DataTableFunctionLibrary)`).

## What you cannot do from text, and how to hand it over

Not scriptable reliably: creating/wiring graph nodes (EventGraph, functions, macros, AnimGraph,
material graphs are a separate case), Widget Blueprint layout, Blueprint Interfaces/Macro
Libraries authoring, timeline editing. First try to remove the need: move logic into C++ and leave
Blueprint as data. When graph work remains, give the human a precise recipe:

```
Asset: /Game/MyGame/Interactables/BP_Door  (open it, Event Graph tab)
1. Right-click empty graph > search "Event On Interacted" > add it.
   (It is the BlueprintImplementableEvent from AInteractableBase.)
2. Drag from the "Interacting Actor" pin > search "Play Sound at Location" > add.
3. Connect exec: On Interacted -> Play Sound at Location.
4. Sound pin: drag from the graph's "Interact Sound" variable (My Blueprint > Variables, show
   inherited variables via the gear icon).
5. Location pin: right-click graph > "Get Actor Location" (Target = self) > connect Return Value.
6. Compile (toolbar), Save.
Expected result: pressing Interact near the door plays SC_DoorOpen at the door.
```
Rules for recipes: give the asset path, the graph/tab, the exact palette search text (node
display names, as shown with Context Sensitive on), pin names as displayed, every connection, the
values of literal pins, and a way to confirm it works. Ask the human to report back when done,
then verify via `ue_python` (compile status, CDO values) and `ue_log`.

## Performance pitfalls

- **Event Tick** in Blueprint on many actors is the classic cost. Disable Tick (Class Defaults >
  Actor Tick > Start with Tick Enabled off, or `PrimaryActorTick.bCanEverTick = false` in the C++
  parent), use timers or events, or set Tick Interval.
- **Casting to Blueprint classes** creates a hard reference: casting to `BP_Boss` from a widget loads
  `BP_Boss` and everything it references whenever the widget loads. Cast to the C++ base class or
  use an interface instead.
- **Hard references in variables and defaults**: a `BP_` variable typed as another Blueprint or an
  asset loads it with the owner. Use soft object/class references for optional or large content;
  check with Size Map and Reference Viewer.
- **Heavy loops** (`ForEachLoop` over hundreds of elements every frame), `Get All Actors Of Class`
  in Tick, string building in Tick: move to C++.
- **Pure nodes** are re-evaluated for every connected input; cache expensive pure results in a local
  variable.
- **Construction Script** runs on every property change and move in the editor; keep it light.
- **Blueprint Nativization was removed in UE 5.0**. The only way to speed up hot Blueprint code is
  to move it to C++.

## Pitfalls

- Renaming/deleting a C++ UPROPERTY/UFUNCTION used by Blueprints without a redirect breaks those
  Blueprints silently until they are opened (compile errors in the log).
- Reparenting a Blueprint loses variables/components that do not exist on the new parent.
- Blueprints with compile errors still save; always check `ue_log` after compiling.
- A C++ `UPROPERTY` with a default changed in C++ does not update Blueprints that already override
  it (the BP value wins; the Details panel shows a reset arrow).
- Circular Blueprint dependencies (BP_A casts to BP_B and vice versa) cause long load/compile
  times; break them with interfaces or C++ bases.

## Relay tools for this work

- `ue_blueprint_info {paths | folder, compile?}` reads Blueprints as text: parent class,
  interfaces, variables with defaults, functions and events, components, and graph nodes where the
  engine exposes them. `compile: true` compiles and returns the compiler's log lines. Read a
  Blueprint before changing it or describing a change to the human.
- `ue_asset_refs` before renaming or deleting a Blueprint.
- `ue_play` to check that the Blueprint behaves at runtime.

## Verify your work

- [ ] Every created/modified asset compiled (no errors in `ue_log`) and saved; the human has the
      list of asset paths.
- [ ] `ue_search_assets` finds the new assets with the expected class.
- [ ] CDO values read back correctly (`ue_python`: `print(cdo.get_editor_property(...))`).
- [ ] Graph work handed over as a numbered recipe with a stated expected result.
- [ ] No new Blueprint-to-Blueprint casts or large hard references introduced without reason.
