---
name: unreal-cpp
description: Writing Unreal Engine 5 C++ - UCLASS/UPROPERTY/UFUNCTION specifiers, Epic's coding standard, containers and strings, delegates, timers, logging, check/ensure, subsystems, interfaces, async tasks, smart pointers, Live Coding, and compile, link and UHT errors. Use whenever creating or editing .h/.cpp files in an Unreal project.
---

# Unreal C++

Load `unreal-fundamentals` first if you have not (modules, UObject/GC, CDO, paths). Deeper
material:

- `reference/specifiers.md` - full tables of UCLASS, USTRUCT, UENUM, UPROPERTY, UFUNCTION
  specifiers and meta keys. Read it whenever you write a macro whose specifier you are not 100%
  sure of.
- `reference/delegates-and-timers.md` - every delegate kind with declare/bind/broadcast code,
  lifetime rules, and the timer manager API. Read it before adding any event, callback or timer.

For gameplay classes (GameMode, Pawn, Character, input, spawning) see `unreal-gameplay-framework`.
For exposing C++ to Blueprint and the C++/Blueprint split see `unreal-blueprints`. Replication
and RPCs: `unreal-multiplayer`. UMG widget C++: `unreal-ui-umg`. Build configurations and
packaging: `unreal-build-packaging`. Profiling: `unreal-performance`.

## Workflow for a C++ change

1. Find the module the code belongs in (`ue_project_info`, `Source/<Module>`). Match its layout
   (`Public/`+`Private/` or flat).
2. Write the header and .cpp. File name = type name without prefix.
3. Add any new module dependencies to `<Module>.Build.cs`.
4. Build: close the editor (or ask the human to) and run `ue_build`. If the change touches only
   .cpp function bodies and the editor is open, the human can press Ctrl+Alt+F11 (Live Coding)
   instead.
5. Read the errors `ue_build` extracts; fix the first error first (later ones are often cascades).
6. Start the editor / check `ue_log` for runtime errors, ensure failures and warnings from your
   log category.

## Class skeleton

```cpp
// Public/Components/HealthComponent.h
#pragma once

#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "HealthComponent.generated.h"

DECLARE_DYNAMIC_MULTICAST_DELEGATE_TwoParams(FOnHealthChangedSignature, float, NewHealth, float, Delta);

UCLASS(ClassGroup = (Combat), meta = (BlueprintSpawnableComponent))
class MYGAME_API UHealthComponent : public UActorComponent
{
    GENERATED_BODY()

public:
    UHealthComponent();

    UFUNCTION(BlueprintCallable, Category = "Health")
    void ApplyHealthDelta(float Delta);

    UFUNCTION(BlueprintPure, Category = "Health")
    float GetHealth() const { return Health; }

    UPROPERTY(BlueprintAssignable, Category = "Health")
    FOnHealthChangedSignature OnHealthChanged;

protected:
    virtual void BeginPlay() override;

    UPROPERTY(EditAnywhere, BlueprintReadOnly, Category = "Health", meta = (ClampMin = "1.0"))
    float MaxHealth = 100.f;

private:
    UPROPERTY(VisibleInstanceOnly, Category = "Health")
    float Health = 0.f;
};
```

```cpp
// Private/Components/HealthComponent.cpp
#include "Components/HealthComponent.h"

UHealthComponent::UHealthComponent()
{
    PrimaryComponentTick.bCanEverTick = false;   // do not tick unless you need to
}

void UHealthComponent::BeginPlay()
{
    Super::BeginPlay();
    Health = MaxHealth;
}

void UHealthComponent::ApplyHealthDelta(float Delta)
{
    const float Old = Health;
    Health = FMath::Clamp(Health + Delta, 0.f, MaxHealth);
    if (Health != Old)
    {
        OnHealthChanged.Broadcast(Health, Health - Old);
    }
}
```

Rules:
- Always call `Super::` in overridden lifecycle functions (`BeginPlay`, `EndPlay`, `Tick`,
  `PostInitializeComponents`, `SetupPlayerInputComponent`...).
- Initialize members in-class (`float MaxHealth = 100.f;`) or in the constructor.
- Turn tick off (`PrimaryActorTick.bCanEverTick = false` / `PrimaryComponentTick`) unless needed.
- Keep UPROPERTYs `protected`/`private` with `BlueprintReadOnly` + getters where possible.
  Private members exposed to Blueprint need `meta = (AllowPrivateAccess = "true")`.

## Macro essentials (see `reference/specifiers.md` for everything)

| Want | Write |
|---|---|
| Editable default on the Blueprint class, not per placed instance | `UPROPERTY(EditDefaultsOnly)` |
| Editable per placed instance too | `UPROPERTY(EditAnywhere)` |
| Visible, not editable (component pointers) | `UPROPERTY(VisibleAnywhere)` |
| Blueprint get / get+set | `BlueprintReadOnly` / `BlueprintReadWrite` (needs `Category`) |
| Not saved | `Transient` |
| Blueprint-callable function with exec pins | `UFUNCTION(BlueprintCallable)` |
| Pure getter node (no exec pins) | `UFUNCTION(BlueprintPure)` |
| Implemented only in Blueprint | `UFUNCTION(BlueprintImplementableEvent)` |
| C++ default, Blueprint may override | `UFUNCTION(BlueprintNativeEvent)` + `Foo_Implementation` |
| Button in Details panel (editor) | `UFUNCTION(CallInEditor)` |

`Category` is required for any Blueprint-exposed property or function in engine and plugin code
and strongly recommended everywhere.

## Containers

```cpp
TArray<int32> Ids;               // contiguous, like std::vector
Ids.Reserve(16);
Ids.Add(3); Ids.Emplace(4); Ids.AddUnique(3);
if (Ids.Contains(4)) { Ids.Remove(4); }          // removes all equal elements, keeps order
Ids.RemoveAtSwap(0);                              // O(1), changes order
const int32 Index = Ids.IndexOfByKey(3);          // INDEX_NONE (-1) if absent
if (Ids.IsValidIndex(Index)) { ... }
Ids.Sort();                                       // or Ids.Sort([](int32 A, int32 B){ return A > B; });
int32* Found = Ids.FindByPredicate([](int32 V){ return V > 2; });
Ids.RemoveAll([](int32 V){ return V < 0; });

TMap<FName, int32> Counts;       // hash map
Counts.Add(TEXT("Apple"), 1);                     // overwrites existing key
int32& C = Counts.FindOrAdd(TEXT("Pear"));
if (int32* P = Counts.Find(TEXT("Apple"))) { ++*P; }
int32 V = Counts.FindRef(TEXT("Kiwi"));           // value copy, default if missing
for (const TPair<FName, int32>& Pair : Counts) { /* Pair.Key, Pair.Value */ }

TSet<FName> Tags;                // hash set
Tags.Add(TEXT("Enemy"));
```
- Never add/remove while range-iterating. Iterate backwards by index, collect then remove, or use
  `RemoveAll`. For maps use `for (auto It = Map.CreateIterator(); It; ++It) { if (...) It.RemoveCurrent(); }`.
- Indices are `int32`, `Num()` returns `int32`.
- `UPROPERTY` supports `TArray`, `TMap`, `TSet` but not nested containers (`TArray<TArray<int32>>`).
  Wrap the inner container in a `USTRUCT`. TMap/TSet UPROPERTYs cannot be replicated.
- Custom struct keys in `TMap`/`TSet` need `operator==` and a `GetTypeHash` overload.
- Use `TArray<TObjectPtr<UFoo>>` inside UPROPERTY for UObject arrays.

## Strings

| Type | Use | Notes |
|---|---|---|
| `FString` | Mutable general string, file paths, debug text | Heap allocated, `TCHAR` |
| `FName` | Identifiers: asset names, bone/socket names, tags, map keys | Case-insensitive, interned, fast compare, immutable |
| `FText` | Anything shown to the player | Localizable; never build UI text from `FString` in shipping content |

```cpp
FString S = FString::Printf(TEXT("Score: %d, Name: %s"), Score, *Name);   // *FString gives const TCHAR*
FName N(TEXT("Muzzle"));  FName N2 = FName(*S);
FString FromName = N.ToString();
FText T = FText::FromString(S);          // non-localized; ok for debug/dynamic data
FString FromText = T.ToString();

#define LOCTEXT_NAMESPACE "MyHUD"
FText Msg = FText::Format(LOCTEXT("AmmoFmt", "Ammo: {0}"), FText::AsNumber(Ammo));
#undef LOCTEXT_NAMESPACE
FText Other = NSLOCTEXT("MyGame", "GameOver", "Game Over");
```
- Always wrap literals in `TEXT()` for `FString`/`FName`.
- `%s` in `Printf`/`UE_LOG` needs `*MyFString`, never the FString itself.
- Conversions: `FString::FromInt`, `FString::SanitizeFloat`, `LexToString(Value)`,
  `FCString::Atoi`, `LexFromString(OutValue, *Str)`. UTF-8 for external APIs: `TCHAR_TO_UTF8(*S)`
  (temporary, do not store the pointer) or `FTCHARToUTF8`.
- `NAME_None` is the empty FName; test with `Name.IsNone()`.

## Logging

```cpp
// MyGame.h (module public header)
DECLARE_LOG_CATEGORY_EXTERN(LogMyGame, Log, All);
// MyGame.cpp
DEFINE_LOG_CATEGORY(LogMyGame);

// anywhere that includes MyGame.h
UE_LOG(LogMyGame, Warning, TEXT("Spawned %s at %s"), *GetNameSafe(Actor), *Location.ToString());
UE_CLOG(bFailed, LogMyGame, Error, TEXT("Load failed for %s"), *Path);

// file-local category
DEFINE_LOG_CATEGORY_STATIC(LogInventory, Log, All);
```
- Verbosity: `Fatal` (crashes), `Error`, `Warning`, `Display`, `Log`, `Verbose`, `VeryVerbose`.
  The second arg of DECLARE is the default runtime verbosity; the third is the compile-time max.
- UE 5.2+: structured logging `UE_LOGFMT(LogMyGame, Log, "Hit {Target} for {Damage}", GetNameSafe(Target), Damage);`
  with `#include "Logging/StructuredLog.h"`.
- Change verbosity at runtime: console `Log LogMyGame Verbose` (via `ue_console`), or
  `[Core.Log]` section in `DefaultEngine.ini`: `LogMyGame=Verbose`.
- On-screen debug: `if (GEngine) GEngine->AddOnScreenDebugMessage(-1, 5.f, FColor::Yellow, TEXT("..."));`
- Read results with `ue_log` using `filter: "LogMyGame"`.

## Assertions

| Macro | Shipping build | On failure | Use for |
|---|---|---|---|
| `check(Expr)` / `checkf(Expr, TEXT("fmt"), ...)` | Compiled out (expression not evaluated) | Crash with callstack | Invariants that must never be false |
| `checkSlow(Expr)` | Only in Debug builds | Crash | Expensive invariants |
| `checkNoEntry()` | Compiled out | Crash if reached | Unreachable branches |
| `verify(Expr)` / `verifyf` | Expression still evaluated, not checked | Crash (non-shipping) | Expressions with side effects |
| `ensure(Expr)` / `ensureMsgf(Expr, TEXT("fmt"), ...)` | Expression evaluated, returns bool | Logs error + callstack once per call site per session, continues | Recoverable bugs: `if (!ensure(Ptr)) return;` |
| `ensureAlways(Expr)` / `ensureAlwaysMsgf` | Evaluated | Reports every time | Rare, when each failure matters |

Never put side effects inside `check()`. Prefer `ensure` + early return in gameplay code so a bug
does not crash the editor for the human. Never use `check` to validate data from assets or users;
handle it and log.

## Subsystems

Engine-managed singletons scoped to a lifetime. Prefer them over manual singletons or stuffing
everything into GameInstance.

| Base class | Lifetime | Access |
|---|---|---|
| `UEngineSubsystem` | Engine (editor + game) | `GEngine->GetEngineSubsystem<T>()` |
| `UEditorSubsystem` (module `EditorSubsystem`, editor modules only) | Editor | `GEditor->GetEditorSubsystem<T>()` |
| `UGameInstanceSubsystem` | Game instance (survives level travel) | `GetGameInstance()->GetSubsystem<T>()`, `UGameInstance::GetSubsystem<T>(GI)` |
| `UWorldSubsystem` / `UTickableWorldSubsystem` | One world (editor worlds too) | `GetWorld()->GetSubsystem<T>()` |
| `ULocalPlayerSubsystem` | One local player | `LocalPlayer->GetSubsystem<T>()`, `ULocalPlayer::GetSubsystem<T>(LP)` |

```cpp
UCLASS()
class MYGAME_API UScoreSubsystem : public UGameInstanceSubsystem
{
    GENERATED_BODY()
public:
    virtual void Initialize(FSubsystemCollectionBase& Collection) override;
    virtual void Deinitialize() override;

    UFUNCTION(BlueprintCallable, Category = "Score")
    void AddScore(int32 Amount) { Score += Amount; }
private:
    int32 Score = 0;
};
```
- Subsystems are created automatically; no registration. Override
  `ShouldCreateSubsystem(UObject* Outer) const` to opt out (e.g. `UWorldSubsystem` for non-game
  worlds: override `DoesSupportWorldType(EWorldType::Type)`).
- Declare init order dependencies with `Collection.InitializeDependency<UOtherSubsystem>();`.
- They are automatically exposed to Blueprint as "Get <Name>" nodes.

## Interfaces

```cpp
UINTERFACE(MinimalAPI, Blueprintable)
class UInteractable : public UInterface { GENERATED_BODY() };

class MYGAME_API IInteractable
{
    GENERATED_BODY()
public:
    UFUNCTION(BlueprintNativeEvent, BlueprintCallable, Category = "Interaction")
    void Interact(AActor* Instigator);
};

// implementer
UCLASS()
class AMyDoor : public AActor, public IInteractable
{
    GENERATED_BODY()
public:
    virtual void Interact_Implementation(AActor* Instigator) override;
};

// caller: works for C++ AND Blueprint implementers
if (Target && Target->Implements<UInteractable>())
{
    IInteractable::Execute_Interact(Target, this);
}
```
- `Cast<IInteractable>(Obj)` only succeeds for C++ implementers. For interfaces Blueprints may
  implement, always use `Implements<U...>()` + `Execute_...`.
- Store references as `TScriptInterface<IInteractable>` in UPROPERTY.
- Interfaces with `BlueprintNativeEvent`/`BlueprintImplementableEvent` functions must be
  `Blueprintable`. Pure C++ interfaces use `UINTERFACE(meta = (CannotImplementInterfaceInBlueprint))`
  with plain virtual functions.

## Non-UObject memory

- `TUniquePtr<T>` + `MakeUnique<T>(...)`: sole ownership.
- `TSharedPtr<T>` / `TSharedRef<T>` (non-null) + `MakeShared<T>(...)`; `TWeakPtr<T>` to observe.
  Thread-safe reference counting is the default in UE5 (`ESPMode::ThreadSafe`).
- `TSharedFromThis<T>` for `AsShared()`. Slate widgets use `TSharedRef`/`SNew`.
- Never hold a UObject in `TSharedPtr`/`TUniquePtr`. Never hold a TSharedPtr in a UPROPERTY
  (unsupported).

## Async and threads

UObjects are not thread safe. Touch them only on the game thread. Pattern: compute on a worker,
return to the game thread to apply.

```cpp
#include "Async/Async.h"

TWeakObjectPtr<AMyActor> WeakThis(this);
Async(EAsyncExecution::ThreadPool, [WeakThis, Input = CopyOfData]()
{
    FResult Result = HeavyCompute(Input);             // no UObject access here
    AsyncTask(ENamedThreads::GameThread, [WeakThis, Result = MoveTemp(Result)]()
    {
        if (AMyActor* Self = WeakThis.Get()) { Self->ApplyResult(Result); }
    });
});
```
- `AsyncTask(ENamedThreads::..., Lambda)` fires a task-graph task. `Async(...)` returns a `TFuture`.
  `UE::Tasks::Launch(UE_SOURCE_LOCATION, Lambda)` (`#include "Tasks/Task.h"`) is the newer tasks
  system. `ParallelFor(Num, [](int32 i){...})` for data-parallel loops.
- `FRunnable` + `FRunnableThread::Create(Runnable, TEXT("Name"))` for a long-lived dedicated
  thread: implement `Init`, `Run`, `Stop`, `Exit`; signal stop with a `std::atomic<bool>` or
  `FThreadSafeBool`, and `Kill(true)`/delete the thread on shutdown.
- For latent gameplay waiting prefer timers (`reference/delegates-and-timers.md`) over threads.

## Live Coding vs full rebuild

- Live Coding (Ctrl+Alt+F11 in the editor, enabled by default in UE5) patches function bodies in
  the running editor. Safe for .cpp logic changes.
- Changes to reflected headers (new/removed UPROPERTY, UFUNCTION, UCLASS, USTRUCT, changed
  specifiers, class layout) need the editor closed and a full `ue_build`, then restart. Live Coding
  may appear to work and then corrupt Blueprint data or crash.
- Hot Reload (UE4 era) is superseded; do not rely on it.
- If the build fails with "Unable to build while Live Coding is active" or locked DLLs, the editor
  is open: ask the human to close it, or to use Live Coding if the change qualifies.

## Include what you use

- Include `CoreMinimal.h` then the specific headers you need; do not include `Engine.h` or
  `EngineMinimal.h` (monolithic, slow, deprecated patterns).
- Forward declare in headers (`class UStaticMeshComponent;`) and include in the .cpp. A
  `TObjectPtr<UFoo>` member needs only a forward declaration.
- Common headers: `GameFramework/Actor.h`, `GameFramework/Character.h`,
  `GameFramework/CharacterMovementComponent.h`, `Components/StaticMeshComponent.h`,
  `Components/CapsuleComponent.h`, `Camera/CameraComponent.h`, `GameFramework/SpringArmComponent.h`,
  `Kismet/GameplayStatics.h`, `Kismet/KismetMathLibrary.h`, `Engine/World.h`, `TimerManager.h`,
  `EnhancedInputComponent.h`, `EnhancedInputSubsystems.h`, `Blueprint/UserWidget.h`,
  `Net/UnrealNetwork.h`. When unsure, grep the engine source for `class ENGINE_API UFoo` /
  `class <MODULE>_API UFoo` to find the header path relative to `Public/` or `Classes/`.
- Unity builds merge .cpp files and hide missing includes; a file that compiles today may break when
  unity grouping changes. To check, build with `bUseUnity = false;` in the `.Build.cs` temporarily
  (revert after), or ask the human to. `IWYUSupport = IWYUSupport.Full;` (5.2+, replaces
  `bEnforceIWYU`) enforces IWYU rules for the module.

## Compile, link and UHT errors

| Symptom | Likely cause |
|---|---|
| `LNK2019`/`undefined reference` to an engine symbol | Module missing from `.Build.cs`; or the class is not exported (`MinimalAPI` classes only export some functions) |
| `LNK2019` to your own class from another module | Missing `MYGAME_API` on the class |
| `'X.generated.h' must be the last include` | Reorder includes |
| `Missing '*' in Expected a pointer type` / `Unrecognized type` in UHT | Type not reflected, not included, or not a supported UPROPERTY type (e.g. `TSharedPtr`, raw nested containers, `std::` types) |
| `BlueprintReadWrite should not be used on private members` | Add `meta = (AllowPrivateAccess = "true")` or make protected |
| `Unable to find 'class', 'delegate', 'enum', or 'struct' with name` | Delegate/struct declared after use, or in a header not included |
| `use of undefined type` / incomplete type | Forward declared but not included in the .cpp |
| `C4458: declaration hides class member` (treated as error) | Rename local/parameter shadowing a member |
| `No matching overloaded function` on `AddDynamic` | Bound function not a `UFUNCTION()` or signature mismatch |
| Unresolved external `X_Implementation` | A `BlueprintNativeEvent` (or RPC `_Implementation`/`_Validate`) was declared but not defined in the .cpp. Call sites call `X(...)` (or `Execute_X` for interfaces), never `X_Implementation` directly from outside |
| Editor crashes on startup after adding a UPROPERTY | Stale binaries vs. Blueprint data; full rebuild with editor closed |

## Common pitfalls

- Using `GetWorld()` in a constructor (null). Use `BeginPlay`.
- `CreateDefaultSubobject` outside the constructor (crash). Use `NewObject` + `RegisterComponent`
  at runtime.
- Binding a delegate to a lambda capturing raw `this` without weak protection.
- Forgetting `Super::` calls.
- Heavy work in `Tick`; prefer events, timers, or reduced `TickInterval`.
- `float` vs `double`: UE5 math types (`FVector`, `FRotator`) are double precision; use `FVector3f`
  only where APIs require float. Literals like `1.f` are fine for float members.
- Comparing floats with `==`; use `FMath::IsNearlyEqual`.

## Verify your work

- [ ] `ue_build` succeeds with no new warnings in your files.
- [ ] Every UObject member pointer is a `UPROPERTY()` (or deliberately weak).
- [ ] Every overridden lifecycle function calls `Super::`.
- [ ] New dependencies are in `.Build.cs`; no runtime module depends on `UnrealEd`.
- [ ] Blueprint-exposed members have `Category`.
- [ ] `ue_log` shows no `Ensure condition failed`, `Error` or `Warning` from your code after a PIE run.
- [ ] If headers changed, the human knows the editor must be restarted (not Live Coded).
