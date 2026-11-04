---
name: unreal-testing-debugging
description: Testing and debugging Unreal Engine 5 - automation tests and specs, running them headless, Functional Tests, Gauntlet, debuggers and DebugGame, crash logs and minidumps, logging, the Visual Logger and Gameplay Debugger, DrawDebug, console variables, cheat managers, ue_log. Use when writing tests, when something crashes, asserts or misbehaves, or when adding debug tooling.
---

# Unreal Testing and Debugging

Two halves: automated tests that prove behaviour, and tools to see what the game is doing.
`reference/automation-tests.md` has complete test templates (simple, complex, spec,
latent, functional, CI command lines) - read it before writing or running tests.

## 1. Where to look first

1. `ue_log {"lines": 300, "filter": "Error|Warning|Fatal|Ensure"}` - `Saved/Logs/<Project>.log`
   (the current session; previous sessions are `<Project>-backup-<timestamp>.log`).
2. Crashes: `Saved/Crashes/<crash folder>/` contains `CrashContext.runtime-xml` (callstack,
   engine version), `UEMinidump.dmp` and a copy of the log. Read the XML and the log's
   last lines with the file tools. Packaged builds on Windows write to
   `%LOCALAPPDATA%/<Project>/Saved/` (Logs, Crashes); Linux to `~/.config/Epic/<Project>/Saved/`.
3. Build errors: see `unreal-build-packaging`. Blueprint compile errors: `ue_log` with
   `filter: "LogBlueprint"`, or the Compiler Results panel.
4. Reproduce, then narrow with logs, breakpoints or a test. Fix the cause, add a test.

## 2. Automation tests (C++)

Tests live in the game module (or a separate test module) and compile only in builds with
`WITH_DEV_AUTOMATION_TESTS` (not Shipping). Put them in `Private/Tests/*.cpp`.

```cpp
// Source/MyGame/Private/Tests/InventoryTests.cpp
#include "Misc/AutomationTest.h"
#include "Inventory/InventoryMath.h"

#if WITH_DEV_AUTOMATION_TESTS

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FInventoryStackTest, "MyGame.Inventory.StackLimit",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::ProductFilter)

bool FInventoryStackTest::RunTest(const FString& Parameters)
{
    TestEqual(TEXT("Adds within limit"), InventoryMath::AddToStack(5, 3, 10), 8);
    TestEqual(TEXT("Clamps at limit"), InventoryMath::AddToStack(9, 5, 10), 10);
    TestFalse(TEXT("Rejects negative"), InventoryMath::IsValidCount(-1));
    return true;   // failures are recorded by the Test* calls
}

#endif
```
- Test name `MyGame.Area.Case` is the hierarchy shown in the UI and used to filter runs.
- **Flags**: one or more *context* flags (`EditorContext`, `ClientContext`,
  `ServerContext`, `CommandletContext`) and exactly one *filter* flag (`SmokeFilter` -
  fast, runs often; `EngineFilter`; `ProductFilter` - project tests; `PerfFilter`;
  `StressFilter`; `NegativeFilter`). The helper masks are
  `EAutomationTestFlags::ApplicationContextMask` before 5.5 and
  `EAutomationTestFlags_ApplicationContextMask` from 5.5 (the enum became an `enum class`);
  check `Misc/AutomationTest.h` in your engine.
- Assertions: `TestEqual`, `TestNotEqual`, `TestTrue`, `TestFalse`, `TestNull`,
  `TestNotNull`, `TestValid`, `TestNearlyEqual` (floats/vectors with tolerance in recent
  versions; `TestEqual` with a tolerance argument also exists), `AddError`, `AddWarning`,
  `AddInfo`. `UTEST_EQUAL(What, A, B)` / `UTEST_TRUE(What, X)` etc. return `false` from
  `RunTest` immediately on failure.
- Expected log errors: `AddExpectedError(TEXT("regex or substring"), EAutomationExpectedErrorFlags::Contains, 1);`
  otherwise an error logged during the test fails it.
- Needs a world? Use a latent command or a spec with `LatentIt`, or a functional test in a
  map (below). Pure logic should be tested without a world: keep game rules in plain
  functions/structs so they are testable.

Spec style (BDD) is better for larger suites:
```cpp
#if WITH_DEV_AUTOMATION_TESTS
BEGIN_DEFINE_SPEC(FWalletSpec, "MyGame.Economy.Wallet",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::ProductFilter)
    FWallet Wallet;
END_DEFINE_SPEC(FWalletSpec)

void FWalletSpec::Define()
{
    BeforeEach([this]() { Wallet = FWallet(100); });
    Describe("Spend", [this]()
    {
        It("reduces the balance", [this]() { Wallet.Spend(30); TestEqual("Balance", Wallet.GetBalance(), 70); });
        It("refuses to go negative", [this]() { TestFalse("Spend 200", Wallet.Spend(200)); });
    });
}
#endif
```
Disable a block temporarily with `xIt` / `xDescribe`. Latent variants (`LatentIt`,
`LatentBeforeEach`) receive an `FDoneDelegate` that you must execute when finished.

## 3. Running tests

- **Editor UI**: Tools > **Test Automation** (older layouts: Tools > Session Frontend >
  Automation tab). Pick tests by name, Start Tests, read results and logs per test.
- **Through the MCP bridge** (editor open): `ue_console {"command": "Automation RunTests MyGame.Inventory"}`
  then read `ue_log {"filter": "LogAutomation|Test Completed|Error"}`. `Automation List`
  logs available tests.
- **Headless / CI** (editor closed; build the editor target first):
```
UnrealEditor-Cmd "<abs>/MyGame.uproject" -ExecCmds="Automation RunTests MyGame;Quit" ^
  -unattended -nullrhi -nopause -nosplash -TestExit="Automation Test Queue Empty" ^
  -ReportExportPath="<abs>/Saved/Automation/Reports" -log
```
  `RunTests <filter>` matches by name prefix/substring; separate several with `+`.
  The report folder gets `index.json` (+ an HTML viewer). Parse `index.json` for
  `succeeded`/`failed` counts, and treat a non-zero exit code as failure.
  On Linux/Mac use the `UnrealEditor` binary with the same arguments.

## 4. Functional tests (in-world)

For gameplay behaviour that needs a real world (movement, AI, physics, spawning):
1. Create a test map, e.g. `/Game/Tests/FTEST_Doors` (human, or `LevelEditorSubsystem.new_level`).
2. Place a **Functional Test** actor (`AFunctionalTest`) or a Blueprint/C++ subclass.
3. In the test, implement the start logic and call **Finish Test** with Succeeded/Failed
   (Blueprint: Event *On Test Start*, node *Finish Test*; C++: override `StartTest()` and call
   `FinishTest(EFunctionalTestResult::Succeeded, TEXT("..."))`). Set a Time Limit so a hung test fails.
4. Tests appear in the Test Automation window under `Project.Functional Tests.<map path>...`;
   run them like other tests (`Automation RunTests Project.Functional Tests`).

C++ subclasses need the `FunctionalTesting` module (a Developer module, absent in
Shipping): put them in a test-only module or keep functional tests in Blueprint. Template in
the reference file.

**Gauntlet** (`RunUAT RunUnreal ...`) launches packaged builds or editors, runs a test
controller, and collects results across devices and multiple processes (client + server).
Use it for smoke tests of packaged builds and multiplayer; it is heavier to set up.

## 5. Debugger

- Build **DebugGame Editor** (`ue_build {"configuration": "DebugGame"}`) to debug your code
  with an optimized engine. Development optimizes your code too, so variables show as
  optimized away. To debug one file in Development, wrap it:
  `UE_DISABLE_OPTIMIZATION` ... `UE_ENABLE_OPTIMIZATION` (5.2+; older:
  `PRAGMA_DISABLE_OPTIMIZATION`). Never commit these.
- Attach: Visual Studio/Rider > Attach to Process > `UnrealEditor.exe` (or the game exe);
  Linux: `gdb -p <pid>` / `lldb -p <pid>`. `-WaitForDebugger` on the command line waits
  at startup until a debugger attaches. The agent can prepare breakpoints/launch configs,
  but a human drives interactive debugging.
- Blueprint: F9 on a node for a breakpoint, watch pins, **Tools > Debug > Blueprint Debugger**.

## 6. Assertions

| Macro | Compiled in Shipping | On failure |
|---|---|---|
| `check(expr)` / `checkf(expr, TEXT("fmt"), ...)` | No (expression not evaluated) | Fatal crash with callstack |
| `verify(expr)` / `verifyf` | Expression **is** evaluated; check removed | Fatal in non-Shipping |
| `ensure(expr)` / `ensureMsgf(expr, TEXT(...))` | No; evaluates to `expr` so it can be used in `if` | Logs callstack once per call site, reports, continues |
| `ensureAlways(expr)` | As ensure | Reports every time |
| `checkNoEntry()` / `unimplemented()` | No | Fatal if reached |
| `checkSlow(expr)` | Only in Debug | Fatal |

Use `check` for programmer invariants that make continuing unsafe; `ensure` for "should not
happen but we can recover"; normal `if` + log for things data or players can cause. Never
put side effects inside `check()`. Pattern: `if (!ensure(Comp)) { return; }`.

## 7. Logging

```cpp
// MyGame.h
DECLARE_LOG_CATEGORY_EXTERN(LogMyGame, Log, All);
// MyGame.cpp
DEFINE_LOG_CATEGORY(LogMyGame);

UE_LOG(LogMyGame, Warning, TEXT("Door %s failed to open: %d"), *GetName(), Reason);
// 5.2+ structured logging
#include "Logging/StructuredLog.h"
UE_LOGFMT(LogMyGame, Warning, "Door {Name} failed to open: {Reason}", GetName(), Reason);
```
One-file categories: `DECLARE_LOG_CATEGORY_STATIC(LogDoorDebug, Log, All);`.
Verbosity: `Fatal`, `Error`, `Warning`, `Display`, `Log`, `Verbose`, `VeryVerbose`.
The default verbosity (second argument) filters at runtime; the compile-time maximum is the third.

Change verbosity:
- Console / `ue_console`: `Log LogMyGame Verbose`, `Log LogMyGame Off`.
- Command line: `-LogCmds="LogMyGame Verbose, LogNet Warning"`.
- Persistent (commit only intentionally), `Config/DefaultEngine.ini`:
```ini
[Core.Log]
LogMyGame=Verbose
LogOnline=Warning
```
Filter the log afterwards with `ue_log {"filter": "LogMyGame"}`. Log what the code
decided and why, with the object name; avoid per-frame logs at `Log` verbosity.

## 8. Seeing the game state

**On-screen messages** (quick, transient; compiled out of Shipping in practice):
```cpp
if (GEngine)
{
    GEngine->AddOnScreenDebugMessage(-1, 5.f, FColor::Yellow,
        FString::Printf(TEXT("Health: %.1f"), Health));
    // A fixed key (e.g. 1) replaces the previous message instead of stacking
    GEngine->AddOnScreenDebugMessage(1, 0.f, FColor::Green, TEXT("State: Chasing"));
}
```
Blueprint: *Print String*.

**Draw debug** (`#include "DrawDebugHelpers.h"`, compiled out when `ENABLE_DRAW_DEBUG` is 0, i.e. Shipping):
```cpp
DrawDebugLine(GetWorld(), Start, End, FColor::Red, false, 2.f, 0, 1.5f);
DrawDebugSphere(GetWorld(), Hit.ImpactPoint, 25.f, 12, FColor::Green, false, 2.f);
DrawDebugBox(GetWorld(), Center, Extent, FColor::Blue, false, 2.f);
DrawDebugPoint(GetWorld(), Loc, 10.f, FColor::White, false, 2.f);
DrawDebugDirectionalArrow(GetWorld(), Start, End, 50.f, FColor::Cyan, false, 2.f);
DrawDebugCapsule(GetWorld(), Center, HalfHeight, Radius, FQuat::Identity, FColor::Orange, false, 2.f);
DrawDebugString(GetWorld(), Loc, TEXT("Target"), nullptr, FColor::White, 2.f);
FlushPersistentDebugLines(GetWorld());
```
Arguments after the colour: `bPersistentLines`, `LifeTime` (seconds; a negative value
means one frame), depth priority, thickness. Gate them behind a cvar (below) so they are
off by default.

**Visual Logger** records per-actor shapes and text over time, for scrubbing after the fact
(AI, movement, combat). `#include "VisualLogger/VisualLogger.h"`:
```cpp
UE_VLOG(this, LogMyGame, Log, TEXT("Chose target %s"), *GetNameSafe(Target));
UE_VLOG_LOCATION(this, LogMyGame, Log, TargetLoc, 30.f, FColor::Red, TEXT("Target"));
UE_VLOG_SEGMENT(this, LogMyGame, Log, GetActorLocation(), TargetLoc, FColor::Yellow, TEXT("Path"));
```
Open **Tools > Debug > Visual Logger** (or console `VisLog`), press Record, play, stop,
then scrub the timeline. Actors can add a snapshot by implementing
`IVisualLoggerDebugSnapshotInterface::GrabDebugSnapshot`.

**Gameplay Debugger**: in PIE press the apostrophe key (`'`) while looking at an actor;
numpad keys toggle categories (AI, Behavior Tree, EQS, Perception, Abilities). Custom
categories derive from `FGameplayDebuggerCategory` and are registered through the
`GameplayDebugger` module inside `#if WITH_GAMEPLAY_DEBUGGER` (add `GameplayDebugger`
to the module's dependencies).

## 9. Console variables, commands, cheats

```cpp
static TAutoConsoleVariable<int32> CVarShowAIDebug(
    TEXT("mygame.AI.ShowDebug"), 0,
    TEXT("Draw AI debug info. 0: off, 1: targets, 2: targets and paths"),
    ECVF_Cheat);

void AMyAIController::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    if (CVarShowAIDebug.GetValueOnGameThread() > 0) { /* DrawDebug... */ }
}

static FAutoConsoleCommand CmdDumpInventory(
    TEXT("mygame.DumpInventory"), TEXT("Log the local player's inventory"),
    FConsoleCommandDelegate::CreateStatic(&DumpInventory));
```
Also `FAutoConsoleVariableRef` (binds a cvar to an existing `static` variable) and
`FAutoConsoleCommandWithWorldAndArgs` (receives `const TArray<FString>&, UWorld*`).
Prefix names with the project (`mygame.`); `ECVF_Cheat` hides them in Shipping.

Cheat manager with Exec functions (typed in the console by the player in non-Shipping builds):
```cpp
UCLASS()
class UMyCheatManager : public UCheatManager
{
    GENERATED_BODY()
public:
    UFUNCTION(Exec) void GiveGold(int32 Amount);
    UFUNCTION(Exec) void GodMode();
};
// AMyPlayerController constructor:
CheatClass = UMyCheatManager::StaticClass();
```
`UFUNCTION(Exec)` also works on the PlayerController, possessed Pawn, HUD, GameMode,
GameInstance and CheatManager - not on arbitrary actors. In multiplayer, cheats run on the
client; forward to the server with a Server RPC (`unreal-multiplayer`).

## 10. Common bugs and what to check

| Symptom | Check |
|---|---|
| Crash with `nullptr` in a UObject member | Missing `UPROPERTY()` so GC freed it; `IsValid()` before use |
| Works in PIE, broken in packaged build | Asset not cooked (string path), editor-only code, `WITH_EDITOR` blocks, config in `Saved/` only |
| Blueprint value ignored | C++ constructor overrides vs BP defaults; `EditDefaultsOnly` vs instance-edited value |
| BeginPlay order issues | Do not assume other actors' BeginPlay ran; use explicit init or events |
| Multiplayer only bug | Authority checks, replication conditions, RPC ownership (`unreal-multiplayer`) |
| Hitch/slowness | `unreal-performance` |

## Relay tools for this work

- `ue_run_tests {filter}` runs automation tests headless (a separate editor process, safe while the
  human's editor is open) and returns pass/fail per test with messages; `in_editor: true` runs them
  in the open editor instead. `filter` is a test path prefix; join several with `+`
  (`MyGame.A+MyGame.B`), since `,` is refused.
- `ue_play` plays the game in the editor for N seconds, takes screenshots, runs a Python probe
  against the game world at checkpoints, and returns the log lines produced while playing
  (errors, warnings, ensures, "Accessed None"). Use it to reproduce runtime bugs and to prove fixes.
- `ue_crash` reads the latest crash report (error, call stack, log tail).
- `ue_log` with a filter for everything else.

**Know when to stop.** If a problem reproduces in an empty level or a fresh template project, only
on one engine version, or its call stack is entirely in engine modules with no project frames,
suspect an engine bug. Spend at most two focused attempts on it, then stop and hand it to the
human: the exact steps to reproduce, the engine version, the log and call stack, and the
workarounds you tried. Do not rewrite working systems to route around a suspected engine bug
without agreement.

## Verify your work

- [ ] New logic has an automation test (pure logic) or a functional test (in-world), and
      the tests pass headless or via `ue_console` + `ue_log`.
- [ ] Tests are inside `#if WITH_DEV_AUTOMATION_TESTS` and use a single filter flag.
- [ ] Debug draws, on-screen messages and verbose logs are behind a cvar or removed.
- [ ] No `UE_DISABLE_OPTIMIZATION`, temporary `ensureAlways`, or verbose `[Core.Log]` entries left in the diff.
- [ ] Crash fixes cite the callstack from `Saved/Crashes` and the fix is covered by a test where practical.
