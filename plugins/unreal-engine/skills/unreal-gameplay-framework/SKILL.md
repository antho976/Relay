---
name: unreal-gameplay-framework
description: The Unreal Engine 5 Gameplay Framework - GameMode and GameState, PlayerController and PlayerState, Pawn, Character and movement, Enhanced Input, cameras, damage, spawning and actor lifecycle, SaveGame, level travel and streaming, World Partition. Use when building player characters, controls, cameras, game rules, spawning, saving and loading, or switching levels.
---

# Gameplay Framework

Prerequisites: `unreal-fundamentals`, `unreal-cpp`. For the Blueprint side of these classes
(data-only BP subclasses, handing graph work to the human) see `unreal-blueprints`. Related:
`unreal-multiplayer` (replication, RPCs, authority), `unreal-ui-umg` (widgets, CommonUI),
`unreal-gas` (abilities, attributes, damage via Gameplay Effects), `unreal-animation`
(character Anim Blueprints), `unreal-ai` (AI controllers), `unreal-level-environment`
(World Partition level work). Deeper material:

- `reference/actor-lifecycle.md` - exact order of constructor, OnConstruction,
  PostInitializeComponents, BeginPlay, EndPlay for spawned, placed, and loaded actors and
  components; tick setup; deferred spawning. Read before writing initialization or teardown code.
- `reference/enhanced-input.md` - Input Actions, Mapping Contexts, triggers, modifiers, full C++
  binding, creating input assets via Python, rebinding. Read before touching controls.

## Who owns what

| Class | Exists on | Lifetime | Put here |
|---|---|---|---|
| `UGameInstance` | Every machine, one per game process | Whole session, survives level travel | Cross-level state, save-system access, online session glue (prefer `UGameInstanceSubsystem`s) |
| `AGameModeBase` / `AGameMode` | Server only (never on clients) | One level | Rules: which classes to spawn, spawn points, win/lose, match flow |
| `AGameStateBase` / `AGameState` | Server + replicated to all | One level | Shared match state everyone sees (score, timer, phase) |
| `APlayerController` | Server + owning client | One level (unless seamless travel) | Input handling policy, UI creation, possession, camera manager |
| `APlayerState` | Server + all clients | One level (copied on seamless travel) | Per-player public state (name, score, team) |
| `APawn` / `ACharacter` | Server + clients | Until destroyed / respawn | Physical representation, movement, per-body input actions |
| `AHUD` | Owning client | With its controller | Legacy canvas HUD; host for UMG widget creation is common |
| `APlayerCameraManager` | Owning client | With its controller | Final camera view, shakes, fades, pitch limits |

Rules of thumb:
- Data that must survive the pawn dying (score, inventory in some designs) goes on PlayerState or
  PlayerController, not the Pawn.
- Match rules go on GameMode; anything clients must display goes on GameState/PlayerState.
- `AGameMode` pairs with `AGameState`, `AGameModeBase` with `AGameStateBase`; mixing them logs
  errors. Use the `Base` versions unless you want GameMode's match states (WaitingToStart,
  InProgress, WaitingPostMatch).
- Even in single player, respect the split; it keeps the door open for multiplayer.

## Setting up a GameMode

```cpp
UCLASS()
class MYGAME_API AMyGameMode : public AGameModeBase
{
    GENERATED_BODY()
public:
    AMyGameMode();
};

AMyGameMode::AMyGameMode()
{
    DefaultPawnClass       = AMyCharacter::StaticClass();
    PlayerControllerClass  = AMyPlayerController::StaticClass();
    PlayerStateClass       = AMyPlayerState::StaticClass();
    GameStateClass         = AMyGameState::StaticClass();
    HUDClass               = AMyHUD::StaticClass();
}
```
To use Blueprint subclasses (so artists can set meshes), make a data-only `BP_MyGameMode` from
`AMyGameMode` and set the class fields there (Python: set `default_pawn_class` etc. on the CDO, see
`unreal-blueprints`). Never `ConstructorHelpers::FClassFinder` hardcoded BP paths unless the
project already does; they break on rename.

Which GameMode runs: World Settings > GameMode Override of the map, else the project default:
```ini
; Config/DefaultEngine.ini
[/Script/EngineSettings.GameMapsSettings]
GlobalDefaultGameMode=/Game/MyGame/Core/BP_MyGameMode.BP_MyGameMode_C
GameDefaultMap=/Game/MyGame/Maps/L_MainMenu.L_MainMenu
EditorStartupMap=/Game/MyGame/Maps/L_Test.L_Test
GameInstanceClass=/Script/MyGame.MyGameInstance
```
Editor path: Project Settings > Project > Maps & Modes. The per-map override is in the map's World
Settings (binary): set via `ue_python`:
```python
import unreal
world = unreal.get_editor_subsystem(unreal.UnrealEditorSubsystem).get_editor_world()
ws = world.get_world_settings()
with unreal.ScopedEditorTransaction("Set GameMode override"):
    ws.set_editor_property("default_game_mode", unreal.EditorAssetLibrary.load_blueprint_class("/Game/MyGame/Core/BP_MyGameMode"))
unreal.get_editor_subsystem(unreal.LevelEditorSubsystem).save_current_level()
```
Useful GameModeBase overrides: `InitGame`, `PostLogin(APlayerController*)`,
`HandleStartingNewPlayer_Implementation`, `ChoosePlayerStart_Implementation`,
`RestartPlayer(AController*)`, `Logout`. Place `APlayerStart` actors in the level for spawn points.

## Character, movement, camera

```cpp
// MyCharacter.h
UCLASS()
class MYGAME_API AMyCharacter : public ACharacter
{
    GENERATED_BODY()
public:
    AMyCharacter();
protected:
    UPROPERTY(VisibleAnywhere, BlueprintReadOnly, Category = "Camera")
    TObjectPtr<USpringArmComponent> CameraBoom;

    UPROPERTY(VisibleAnywhere, BlueprintReadOnly, Category = "Camera")
    TObjectPtr<UCameraComponent> FollowCamera;
};

// MyCharacter.cpp
#include "MyCharacter.h"
#include "Camera/CameraComponent.h"
#include "Components/CapsuleComponent.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "GameFramework/SpringArmComponent.h"

AMyCharacter::AMyCharacter()
{
    GetCapsuleComponent()->InitCapsuleSize(42.f, 96.f);

    // third-person: character faces movement direction, camera follows controller
    bUseControllerRotationPitch = false;
    bUseControllerRotationYaw   = false;
    bUseControllerRotationRoll  = false;
    UCharacterMovementComponent* Move = GetCharacterMovement();
    Move->bOrientRotationToMovement = true;
    Move->RotationRate   = FRotator(0.f, 500.f, 0.f);
    Move->MaxWalkSpeed   = 500.f;
    Move->JumpZVelocity  = 700.f;
    Move->AirControl     = 0.35f;

    CameraBoom = CreateDefaultSubobject<USpringArmComponent>(TEXT("CameraBoom"));
    CameraBoom->SetupAttachment(RootComponent);
    CameraBoom->TargetArmLength = 400.f;
    CameraBoom->bUsePawnControlRotation = true;      // arm follows controller rotation

    FollowCamera = CreateDefaultSubobject<UCameraComponent>(TEXT("FollowCamera"));
    FollowCamera->SetupAttachment(CameraBoom, USpringArmComponent::SocketName);
    FollowCamera->bUsePawnControlRotation = false;   // the arm already rotates
}
```
- First-person variant: camera attached to the capsule or mesh head socket,
  `FollowCamera->bUsePawnControlRotation = true`, `bUseControllerRotationYaw = true`,
  `bOrientRotationToMovement = false`.
- Movement input: `AddMovementInput(Direction, Scale)`; look: `AddControllerYawInput`,
  `AddControllerPitchInput`; jump: `Jump()` / `StopJumping()`; crouch needs
  `GetCharacterMovement()->GetNavAgentPropertiesRef().bCanCrouch = true` then `Crouch()`/`UnCrouch()`.
- Custom movement belongs in a `UCharacterMovementComponent` subclass
  (`AMyCharacter::AMyCharacter(const FObjectInitializer& OI) : Super(OI.SetDefaultSubobjectClass<UMyMoveComp>(ACharacter::CharacterMovementComponentName))`),
  not in Tick on the character, especially for multiplayer (prediction).
- Mesh: `GetMesh()` is the `USkeletalMeshComponent`; typical offset
  `GetMesh()->SetRelativeLocationAndRotation(FVector(0,0,-96), FRotator(0,-90,0))` so the
  mesh's feet sit at the capsule bottom and it faces +X. Asset and Anim Blueprint are set in the
  Blueprint subclass.
- Camera manager: `PlayerCameraManagerClass` on the PlayerController; pitch limits via
  `PlayerCameraManager->ViewPitchMin/ViewPitchMax`; switch view with
  `PC->SetViewTargetWithBlend(OtherActor, 0.5f)`; shakes with
  `PC->ClientStartCameraShake(UMyShake::StaticClass())` (`UCameraShakeBase` subclasses).

## Input

UE5 uses **Enhanced Input** (legacy Action/Axis mappings are deprecated since 5.1). Summary:
Input Action assets (`IA_`) describe *what* (Jump, Move as Axis2D); Input Mapping Context assets
(`IMC_`) map keys to actions with modifiers and triggers; the local player's
`UEnhancedInputLocalPlayerSubsystem` holds active contexts; the pawn/controller binds actions to
functions in `SetupPlayerInputComponent`/`SetupInputComponent`.

```cpp
void AMyCharacter::SetupPlayerInputComponent(UInputComponent* PlayerInputComponent)
{
    Super::SetupPlayerInputComponent(PlayerInputComponent);
    if (UEnhancedInputComponent* EIC = Cast<UEnhancedInputComponent>(PlayerInputComponent))
    {
        EIC->BindAction(MoveAction, ETriggerEvent::Triggered, this, &AMyCharacter::Move);
        EIC->BindAction(JumpAction, ETriggerEvent::Started,   this, &ACharacter::Jump);
        EIC->BindAction(JumpAction, ETriggerEvent::Completed, this, &ACharacter::StopJumping);
    }
}
```
Adding the mapping context, the Move function, asset creation and pitfalls:
`reference/enhanced-input.md`. Module: add `"EnhancedInput"` to `.Build.cs`.

## UI and HUD

Create UMG widgets from the PlayerController (local only):
```cpp
// header: UPROPERTY(EditDefaultsOnly, Category="UI") TSubclassOf<UUserWidget> HUDWidgetClass;
//         UPROPERTY() TObjectPtr<UUserWidget> HUDWidget;
void AMyPlayerController::BeginPlay()
{
    Super::BeginPlay();
    if (IsLocalController() && HUDWidgetClass)
    {
        HUDWidget = CreateWidget<UUserWidget>(this, HUDWidgetClass);
        HUDWidget->AddToViewport();
    }
}
```
Input mode: `SetInputMode(FInputModeGameOnly())`, `FInputModeUIOnly`, `FInputModeGameAndUI`, plus
`SetShowMouseCursor(true)` for menus. Modules: `UMG`, `Slate`, `SlateCore`.

## Damage basics

The built-in damage pipeline is simple and fine for many games (GAS is the heavier
alternative; see `unreal-gas`):
```cpp
#include "Kismet/GameplayStatics.h"
UGameplayStatics::ApplyDamage(Target, 20.f, GetInstigatorController(), this, UDamageType::StaticClass());
UGameplayStatics::ApplyPointDamage(Target, 20.f, ShotDir, Hit, GetInstigatorController(), this, UDamageType::StaticClass());
UGameplayStatics::ApplyRadialDamage(this, 50.f, Origin, 300.f, UDamageType::StaticClass(), IgnoreActors, this, GetInstigatorController(), /*bDoFullDamage*/ false);

// receiver: override AActor::TakeDamage (or bind OnTakeAnyDamage)
float AMyCharacter::TakeDamage(float DamageAmount, FDamageEvent const& DamageEvent,
                               AController* EventInstigator, AActor* DamageCauser)
{
    const float Applied = Super::TakeDamage(DamageAmount, DamageEvent, EventInstigator, DamageCauser);
    Health->ApplyHealthDelta(-Applied);
    return Applied;
}
```
Apply damage on the server/authority only. The target must have `bCanBeDamaged` true (default).
Radial damage is blocked by geometry on the `ECC_Visibility` channel by default.

## Spawning

```cpp
FActorSpawnParameters Params;
Params.Owner = this;
Params.Instigator = GetInstigator();
Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AdjustIfPossibleButAlwaysSpawn;
AProjectile* P = GetWorld()->SpawnActor<AProjectile>(ProjectileClass, SpawnTransform, Params);

// deferred: set properties before BeginPlay / construction script runs
AProjectile* D = GetWorld()->SpawnActorDeferred<AProjectile>(ProjectileClass, SpawnTransform, this, GetInstigator(),
                                                             ESpawnActorCollisionHandlingMethod::AlwaysSpawn);
if (D)
{
    D->Speed = 3000.f;
    D->FinishSpawning(SpawnTransform);
}
```
- `ProjectileClass` is `UPROPERTY(EditDefaultsOnly) TSubclassOf<AProjectile>` pointed at the BP.
- `SpawnActor` returns null if collision handling says don't spawn and the spot is blocked, or if
  the class is null/abstract. Always null-check.
- Blueprint equivalent: "Spawn Actor from Class", with `ExposeOnSpawn` properties as pins.
- Destroy with `Destroy()`; set `InitialLifeSpan` or `SetLifeSpan()` for auto-destroy.
- Full ordering of constructor / OnConstruction / BeginPlay: `reference/actor-lifecycle.md`.

## SaveGame

```cpp
UCLASS()
class MYGAME_API UMySaveGame : public USaveGame
{
    GENERATED_BODY()
public:
    UPROPERTY() int32 Version = 1;
    UPROPERTY() FTransform PlayerTransform;
    UPROPERTY() TArray<FName> UnlockedLevels;
    UPROPERTY() TMap<FName, int32> Inventory;
    UPROPERTY() TSoftClassPtr<AActor> EquippedWeaponClass;   // store paths, never live object pointers
};

// save (sync)
UMySaveGame* Save = Cast<UMySaveGame>(UGameplayStatics::CreateSaveGameObject(UMySaveGame::StaticClass()));
Save->PlayerTransform = Pawn->GetActorTransform();
const bool bOk = UGameplayStatics::SaveGameToSlot(Save, TEXT("Slot0"), /*UserIndex*/ 0);

// load (sync)
if (UGameplayStatics::DoesSaveGameExist(TEXT("Slot0"), 0))
{
    if (UMySaveGame* Loaded = Cast<UMySaveGame>(UGameplayStatics::LoadGameFromSlot(TEXT("Slot0"), 0))) { ... }
}

// async (preferred during gameplay; callback on the game thread)
FAsyncSaveGameToSlotDelegate Saved;
Saved.BindUObject(this, &UMySaveSubsystem::OnSaved);   // void OnSaved(const FString& Slot, const int32 UserIndex, bool bSuccess)
UGameplayStatics::AsyncSaveGameToSlot(Save, TEXT("Slot0"), 0, Saved);

FAsyncLoadGameFromSlotDelegate LoadedDel;
LoadedDel.BindUObject(this, &UMySaveSubsystem::OnLoaded); // void OnLoaded(const FString& Slot, const int32 UserIndex, USaveGame* Data)
UGameplayStatics::AsyncLoadGameFromSlot(TEXT("Slot0"), 0, LoadedDel);
```
- Only `UPROPERTY` members are saved. Actor/component pointers are not meaningful across runs:
  save identifiers (FName ids, soft paths, transforms) and rebuild.
- Include a version field and handle old versions on load.
- Files land in `Saved/SaveGames/<Slot>.sav` in editor/PIE on desktop.
- Put save/load orchestration in a `UGameInstanceSubsystem`.
- To save arbitrary actor state, mark fields `UPROPERTY(SaveGame)` and serialize with an
  `FObjectAndNameAsStringProxyArchive` whose `ArIsSaveGame = true` into a `TArray<uint8>`.

## Level travel and streaming

- `UGameplayStatics::OpenLevel(this, FName("L_Level2"))` (short name or full path
  `/Game/Maps/L_Level2`), or `OpenLevelBySoftObjectPtr(this, LevelSoftPtr)` with a
  `TSoftObjectPtr<UWorld>` property (rename-safe). This is a hard travel: every actor, including
  GameMode, PlayerController and Pawn, is destroyed; only `UGameInstance` (and its subsystems)
  persist. Options (appended after `?`): `OpenLevel(this, Name, true, TEXT("listen"))` to host.
- Multiplayer: server calls `GetWorld()->ServerTravel(TEXT("/Game/Maps/L_Arena?listen"))`. With
  `bUseSeamlessTravel = true` on the GameMode, clients stay connected through a transition map
  (`TransitionMap=` in `[/Script/EngineSettings.GameMapsSettings]`). Seamless travel keeps
  PlayerControllers and PlayerStates (`APlayerState::CopyProperties` carries data), plus actors
  added in `GetSeamlessTravelActorList`. Seamless travel has historically not been supported in
  PIE; test it in a Standalone Game or packaged build.
- Level streaming (non World Partition maps): sublevels in the Levels window, loaded with
  `UGameplayStatics::LoadStreamLevel(this, LevelName, bMakeVisibleAfterLoad, bShouldBlockOnLoad, LatentInfo)`
  / `UnloadStreamLevel`, `LoadStreamLevelBySoftObjectPtr`, streaming volumes, or dynamically
  with `ULevelStreamingDynamic::LoadLevelInstance`.
- Add every map you travel to in packaged builds to Project Settings > Packaging > "List of maps
  to include in a packaged build" (or they may not be cooked).

## World Partition awareness

New open-world maps (and the UE5 Open World template) use World Partition:
- The map is a grid of streaming cells; actors load around **streaming sources** (players by
  default). An actor far from the player is simply not in the world: `GetAllActorsOfClass`,
  `ue_level_actors` and editor scripts only see loaded actors. Actors that must always exist:
  uncheck "Is Spatially Loaded" (`bIsSpatiallyLoaded`) in their Details, or put them in an
  always-loaded Data Layer.
- One File Per Actor: each placed actor is its own `.uasset` under
  `Content/__ExternalActors__/<map path>/`. Placing/moving actors changes those files, not the
  `.umap`; report them to the human (source control shows them as hashed file names).
- No sublevels: use **Data Layers** (5.1+: Data Layer Assets + instances) to toggle groups of
  actors at runtime, and **Level Instances** / Packed Level Actors for reusable chunks.
- In the editor, regions must be loaded (World Partition window > select cells > Load Region
  From Selection) before their actors can be edited by script or hand.
- HLODs are built via Build > Build HLODs (or the `WorldPartitionBuilderCommandlet`); stale HLODs
  are a visual issue, not a gameplay one.
- Detect: `World->GetWorldPartition() != nullptr` in C++, or World Settings > Enable World
  Partition in the editor.

## Common pitfalls

- Accessing GameMode on a client (`GetAuthGameMode()` returns null there).
- Adding the mapping context before the pawn has a local player / controller (see
  `reference/enhanced-input.md`).
- Camera flips or doesn't rotate: wrong combination of `bUsePawnControlRotation`,
  `bUseControllerRotationYaw`, `bOrientRotationToMovement`.
- Storing `AActor*` in SaveGame.
- Using `OpenLevel` and expecting PlayerState/Pawn data to survive.
- Spawning in the constructor or `OnConstruction` (runs in editor, spawns stray actors). Spawn in
  `BeginPlay` or later.
- Pawn not possessed: `AutoPossessPlayer` on a placed pawn, or GameMode `DefaultPawnClass` plus a
  `PlayerStart`.

## Verify your work

- [ ] `ue_build` passes; GameMode/controller/pawn classes are set in config or World Settings.
- [ ] PIE run (human, or `ue_console` commands where appropriate): the correct pawn spawns and is
      possessed; `ue_log` shows no `Accessed None`, `Error` or failed ensures.
- [ ] Input: every IA is in an IMC that is added for the local player; bindings fire (temporary
      `UE_LOG` in the handler).
- [ ] Save/load round-trips (write, restart PIE, read) and handles a missing slot.
- [ ] Maps used by travel are in the packaging map list.
- [ ] Changed binary assets (BPs, maps, external actors) saved and listed for the human.
