---
name: unreal-multiplayer
description: Unreal Engine 5 networking - authority and roles, property replication, RPCs, ownership and relevancy, update rates, dormancy, Push Model and Iris, movement prediction, replicating GAS, sessions and Online Subsystems, PIE testing with net emulation, dedicated servers. Use for any replicated gameplay, co-op, PvP, lobby, desync or "works on the server, not the client" bug.
---

# Unreal Multiplayer and Replication

Unreal networking is **server-authoritative client-server**. One server (dedicated, or a
listen server that is also a player) owns the true game state; clients send input/requests
and receive replicated state. There is no peer-to-peer gameplay and no automatic lockstep.

Read `reference/replication-cookbook.md` for copy-ready patterns (health with OnRep, fire
RPC, multicast cosmetics, owner-only data, FastArray inventory, push model, dormancy,
subobjects, CharacterMovement custom flags). Related: `unreal-gas` (ability replication),
`unreal-gameplay-framework` (GameMode/GameState/PlayerState/Controller - which exist where),
`unreal-cpp`, `unreal-build-packaging` (packaging server and client builds).

## Where things exist

| Class | Server | Owning client | Other clients |
|---|---|---|---|
| GameMode | yes | no | no |
| GameState | yes | yes | yes |
| PlayerController | yes | own only | no |
| PlayerState | yes | yes (all) | yes (all) |
| Pawn/Character | yes | yes | yes (if relevant) |
| HUD / UMG widgets | no (unless listen host) | own only | no |

Consequences: never read `GetGameMode()` on a client (null). Put shared match state in
GameState, per-player public state in PlayerState, private per-player state on the
PlayerController or with `COND_OwnerOnly`.

## Authority and roles

- `HasAuthority()` - true on the machine that owns the authoritative copy (server for
  replicated actors; also true on clients for actors they spawned locally that are not replicated).
- `GetLocalRole()`: `ROLE_Authority`, `ROLE_AutonomousProxy` (the locally controlled pawn on
  its owning client), `ROLE_SimulatedProxy` (everyone else's view). `GetRemoteRole()` is the other side.
- `IsLocallyControlled()` (Pawn) / `IsLocalController()` - "is this my player on this
  machine". Use for camera, input, UI, first-person meshes.
- `GetNetMode()`: `NM_Standalone`, `NM_DedicatedServer`, `NM_ListenServer`, `NM_Client`.
  `IsNetMode(NM_DedicatedServer)` to skip cosmetics on a dedicated server.
- Rule: gameplay state changes happen only where `HasAuthority()` is true. Clients request
  changes with Server RPCs.

## Property replication

1. Actor replicates: `bReplicates = true;` in the constructor (or `SetReplicates(true)` at
   runtime on the server). Components: `SetIsReplicatedByDefault(true)` in the component
   constructor, or `SetIsReplicated(true)`.
2. Mark properties:
   ```cpp
   UPROPERTY(Replicated) int32 Ammo;
   UPROPERTY(ReplicatedUsing = OnRep_Health) float Health;
   UFUNCTION() void OnRep_Health(float OldHealth);  // old-value param is optional
   ```
3. Register them:
   ```cpp
   #include "Net/UnrealNetwork.h"
   void AMyActor::GetLifetimeReplicatedProps(TArray<FLifetimeProperty>& OutLifetimeProps) const
   {
       Super::GetLifetimeReplicatedProps(OutLifetimeProps);
       DOREPLIFETIME(AMyActor, Health);
       DOREPLIFETIME_CONDITION(AMyActor, Ammo, COND_OwnerOnly);
   }
   ```
   Forgetting `Super::` breaks replication of base-class properties (including movement).
   A `Replicated` property not registered here triggers an assert/error in the log.

Facts that bite:
- Replication is **server -> client only**, and only for changed values, at the actor's
  update rate. Clients setting a replicated property only change their local copy.
- `OnRep_` functions run **only on clients** in C++. On a listen server, call the handler
  manually after setting the value if the host needs the same reaction. (Blueprint RepNotify
  also fires on the server; C++ does not.)
- Order of property arrival across different actors is not guaranteed. An OnRep can run
  before BeginPlay or before a referenced actor has replicated (pointer is null, arrives later).
- Only the latest value is sent; intermediate values are lost. Do not use replicated
  properties as event queues - use a counter, a FastArray, or an RPC.
- Replicated `UObject*` references only resolve if the referenced object is itself
  replicated or is a stably-named asset/level object.

Conditions (`ELifetimeCondition`): `COND_None`, `COND_InitialOnly`, `COND_OwnerOnly`,
`COND_SkipOwner`, `COND_SimulatedOnly`, `COND_AutonomousOnly`, `COND_SimulatedOrPhysics`,
`COND_InitialOrOwner`, `COND_Custom` (toggle with `DOREPLIFETIME_ACTIVE_OVERRIDE` in
`PreReplication`), `COND_Never`. RepNotify policy: `DOREPLIFETIME_CONDITION_NOTIFY(C, P, COND_None, REPNOTIFY_Always)`
fires OnRep even when the value did not change (needed for predicted values).

## RPCs

```cpp
UFUNCTION(Server, Reliable, WithValidation) void ServerFire(FVector_NetQuantize Origin, FVector_NetQuantizeNormal Dir);
UFUNCTION(Client, Reliable)                 void ClientShowHitMarker();
UFUNCTION(NetMulticast, Unreliable)         void MulticastPlayImpact(FVector_NetQuantize Location);
```
Implement `ServerFire_Implementation(...)`, and `bool ServerFire_Validate(...)` when
`WithValidation` is present (returning false disconnects the client - use only for cheating,
not for gameplay "cannot fire now" checks). `WithValidation` is optional in UE5.

| Called on | Server RPC | Client RPC | NetMulticast |
|---|---|---|---|
| Server | runs on server | runs on owning client | runs on server + all relevant clients |
| Owning client | runs on server | runs locally | runs locally only |
| Non-owning client | **dropped** | runs locally | runs locally only |

- A client can only call Server RPCs on actors **its connection owns**: its PlayerController,
  its possessed Pawn, its PlayerState, and actors whose Owner chain leads to its
  PlayerController (`SetOwner(PC)` on the server). Calling a Server RPC on a world actor (a
  door) from a client silently does nothing - route it through the Pawn/Controller.
- Reliable: guaranteed, ordered; overuse saturates the reliable buffer and disconnects the
  client. Use for rare, important events (fire request, purchase, chat). Never call a
  Reliable RPC every Tick.
- Unreliable: may drop; use for frequent cosmetic/transient data.
- Multicast is for one-off cosmetic events to relevant clients. Late joiners and clients
  that become relevant later do **not** receive it - persistent state must be a replicated property.
- RPC params are value-serialized; keep them small (use `FVector_NetQuantize*`, bytes, tags).

## Ownership and relevancy

- Owner set by `SetOwner()` on the server, or via `SpawnActor` params (`Params.Owner`).
  Possession sets the Pawn's owner to the Controller.
- Relevancy decides whether an actor replicates to a connection at all: by default distance
  (`NetCullDistanceSquared`, default 225,000,000 = 150 m), plus `bAlwaysRelevant`,
  `bOnlyRelevantToOwner`, `bNetUseOwnerRelevancy`, or override `IsNetRelevantFor`.
- When a spawned actor stops being relevant, its channel closes and the client destroys its
  copy (level-placed actors may instead remain and just stop updating). When it becomes
  relevant again it is re-created with current replicated state and OnReps fire again. Do
  not keep client-side raw pointers to other players' actors across relevancy changes.

## Bandwidth: frequency, priority, dormancy, push model

- `NetUpdateFrequency` (checks per second, default 100 for actors, low for PlayerState) and
  `MinNetUpdateFrequency` (adaptive floor). In 5.5+ use `SetNetUpdateFrequency()` /
  `SetMinNetUpdateFrequency()` - direct member access is deprecated. Lower it for things
  that rarely change (pickups: 2-10), keep pawns high.
- `NetPriority` - share of bandwidth when saturated (pawns 3.0, PlayerController 3.0, default 1.0).
- `ForceNetUpdate()` - send changes this frame for a low-frequency actor after an important change.
- Dormancy (`NetDormancy`): `DORM_Never`, `DORM_Awake`, `DORM_DormantAll`, `DORM_DormantPartial`,
  `DORM_Initial` (placed actors that do not change until touched). A dormant actor is not
  checked for changes at all. Before changing a property on a dormant actor call
  `FlushNetDormancy()`, or wake with `SetNetDormancy(DORM_Awake)`. Great for doors, chests,
  destructibles.
- Push Model: properties are only compared when you mark them dirty, cutting server CPU.
  Needs `NetCore` in Build.cs, `FDoRepLifetimeParams` with `bIsPushBased = true`, and
  `MARK_PROPERTY_DIRTY_FROM_NAME(Class, Prop, this)` at every write. Check that it is active
  with the console variable `net.IsPushModelEnabled`. Pattern in the cookbook.
- Replication Graph (plugin) for large player counts replaces per-actor relevancy with spatial
  grids - worth it only for big-world, many-player games.

## Iris

Iris is the newer replication system (opt-in). It shipped as Experimental in 5.1 and has been
moving toward Beta/production in later 5.x releases - check the release notes for the exact
status in the project's engine version before adopting it. Enabling involves the Iris
plugin, `bUseIris = true` in the Target.cs, `SetupIrisSupport(Target);` in module Build.cs
files, and `net.Iris.UseIrisReplication=1`. Most gameplay code (UPROPERTY replication,
RPCs) stays the same; custom `NetSerialize`/`NetDeltaSerialize` and some legacy features
need Iris-specific serializers. Do not switch an existing project to Iris as a side effect
of another task; propose it to the human.

## Movement replication

- `ACharacter` + `UCharacterMovementComponent` (CMC) provides client-side prediction, server
  reconciliation and smoothing for simulated proxies. Drive it with `AddMovementInput`,
  `Jump`, `Crouch` on the owning client - never set the actor location directly for
  locally controlled characters.
- Changing movement speed only on the client (sprint) or only on the server causes
  rubber-banding (corrections). Networked movement modifiers must be in the CMC's saved
  moves: subclass `FSavedMove_Character` and `FNetworkPredictionData_Client_Character`, use a
  custom compressed flag (`FLAG_Custom_0`..`3`), and apply it in `UpdateFromCompressedFlags`
  and `GetMaxSpeed`. See the cookbook. Setting `MaxWalkSpeed` from a replicated property is
  a known source of corrections under lag.
- Non-character actors: `SetReplicateMovement(true)` replicates transform + velocity
  (`ReplicatedMovement`) from server to clients without prediction. Physics objects replicate
  via physics replication settings (smoothing, not prediction).
- Root motion from montages is replicated for Characters when the montage is played through
  the CMC-aware path (and GAS `PlayMontageAndWait`).
- Mover (experimental plugin, 5.4+) is Epic's next-generation movement framework; do not use
  in production unless the human chooses to.
- Debug: `p.NetShowCorrections 1` draws server corrections.

## Replicating GAS

See `unreal-gas`. Key points: ASC `SetIsReplicated(true)`; replication mode Mixed for
players (owner must be the controller), Minimal for AI; `InitAbilityActorInfo` on server
(`PossessedBy`) and client (`OnRep_PlayerState`); PlayerState net update frequency ~100;
grant abilities on server only; attributes use `REPNOTIFY_Always` + `GAMEPLAYATTRIBUTE_REPNOTIFY`.

## Spawning and destroying

- Spawn replicated actors **on the server only**; they appear on clients automatically.
  Spawning on a client creates a local-only actor the server knows nothing about.
- Projectiles: spawn on the server with `bReplicates = true` and replicated movement; for
  responsiveness, optionally spawn a cosmetic local-only copy on the firing client.
- `Destroy()` on the server removes it everywhere. Set `SetLifeSpan()` for fire-and-forget.
- Subobjects (UObjects owned by an actor, e.g. inventory items): 5.1+ use the registered
  subobject list (`bReplicateUsingRegisteredSubObjectList = true`, `AddReplicatedSubObject`).
  See cookbook.

## Sessions, Online Subsystem and travel

- Direct connect (no subsystem needed): host runs `open MapName?listen`; client runs
  `open 192.168.1.10` (default port 7777). Good for development and LAN.
- Online Subsystem (OSS): `Online::GetSessionInterface(GetWorld())` or
  `IOnlineSubsystem::Get()->GetSessionInterface()`; Build.cs `OnlineSubsystem`,
  `OnlineSubsystemUtils`. Flow: `CreateSession` -> (host) `ServerTravel`/`OpenLevel ?listen`;
  client `FindSessions` -> `JoinSession` -> `GetResolvedConnectString` -> `ClientTravel`.
  Bind the `On*Complete` delegates before calling, and clear them in the callback.
- `DefaultEngine.ini` for local development with the Null subsystem:
  ```ini
  [OnlineSubsystem]
  DefaultPlatformService=Null
  ```
  Enable plugin `OnlineSubsystemNull` (on by default in most templates).
- Steam: plugins `OnlineSubsystemSteam` (and `SteamSockets` for the socket net driver),
  `DefaultPlatformService=Steam`, `[OnlineSubsystemSteam] bEnabled=true SteamDevAppId=480` for
  testing, plus the net driver definition. Steam must be running; it does not work in PIE
  (use Standalone). Follow the engine docs page "Online Subsystem Steam" for the exact ini.
- EOS: `OnlineSubsystemEOS` plugin (or the newer Online Services plugins); needs Epic Dev
  Portal product/sandbox/deployment IDs and client credentials in Project Settings.
- CommonUser plugin (`UCommonSessionSubsystem`, `UCommonUserSubsystem`) ships with the Lyra
  sample, not the engine; copy it from Lyra if the project wants its higher-level API.
- Seamless travel (`bUseSeamlessTravel = true` on the GameMode + a transition map) keeps
  connections and selected actors across map changes; PIE does not support it - test in Standalone.
  Test all session code in Standalone/packaged builds with separate processes.

## Testing in the editor (PIE)

Click-path: Play button dropdown (three dots next to Play) > Multiplayer Options:
- **Number of Players**: 2-3.
- **Net Mode**: `Play Standalone` (no networking), `Play As Listen Server` (first window is
  the host), `Play As Client` (a dedicated server runs in the background; every window is a client).
  Always test **Play As Client** - listen-server-only testing hides bugs because the host has authority.
- Editor Preferences > Level Editor > Play > Multiplayer Options: **Run Under One Process**
  (off = separate processes, more realistic), **Enable Network Emulation** with profiles
  (Average/Bad) or custom lag/loss.
- Console alternatives (`ue_console` targets the editor world; in PIE type into the game
  window console): `NetEmulation.PktLag 150`, `NetEmulation.PktLagVariance 30`,
  `NetEmulation.PktLoss 5`, `NetEmulation.Off`.
- Inspect: `stat net`, `stat game`, `log LogNet Verbose`, `p.NetShowCorrections 1`,
  Networking Insights in Unreal Insights (run with `-trace=default,net -NetTrace=1`).
- Read `ue_log` with filter `LogNet|LogNetPlayerMovement|LogRep|Error` after a session.

## Dedicated servers

- Add `Source/<Project>Server.Target.cs`:
  ```csharp
  using UnrealBuildTool;
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
  Match `DefaultBuildSettings`/`IncludeOrderVersion` to the existing Game target.
- **Server targets require a source-built engine**; the Launcher (installed) engine cannot
  build them. Check `EngineAssociation` via `ue_project_info` - a GUID usually means a
  source build, a version number like `5.4` means Launcher. Tell the human if it is Launcher.
- Build: `ue_build` with `{"target": "MyGameServer", "configuration": "Development"}`;
  package with RunUAT `BuildCookRun -server -noclient -serverplatform=Linux` (or Win64).
- Run: `MyGameServer.exe /Game/Maps/Arena -log -port=7777`; connect with `open 127.0.0.1:7777`.
- Guard cosmetic-only code (`#if !UE_SERVER`, or `IsNetMode(NM_DedicatedServer)` checks) and
  never load UI, audio or heavy VFX on dedicated servers.
- Default server map: Project Settings > Maps & Modes > Server Default Map.

## Common bugs and their causes

| Symptom | Cause |
|---|---|
| Works in Standalone / as host, not on clients | Logic ran only where `HasAuthority()`, or state not replicated, or OnRep not called on host |
| Server RPC never runs | Actor not owned by calling client's connection; actor not replicated; called before possession |
| Value correct on server, stale on client | Missing `DOREPLIFETIME`, missing `Super::GetLifetimeReplicatedProps`, actor dormant, not relevant |
| OnRep not firing | Value set on client, value unchanged (use `REPNOTIFY_Always`), property not registered |
| Rubber-banding | Movement changed outside CMC saved moves; server/client speed mismatch; teleporting owned pawn on client |
| Late joiners miss state | State sent via Multicast instead of replicated property |
| Client crash on null in OnRep/BeginPlay | Referenced actor not replicated yet; check for null and handle the later OnRep |
| Disconnect "reliable buffer overflow" | Reliable RPCs in Tick or in loops |
| Duplicate actors | Spawning replicated actors on both server and client |
| UI shows wrong player | Using `GetPlayerController(0)` on a listen server/split screen instead of the owning controller |

## Verify your work

- [ ] Every replicated property registered in `GetLifetimeReplicatedProps`, `Super` called.
- [ ] State changes happen on the server; clients request via Server RPC on owned actors.
- [ ] No Reliable RPC per frame; cosmetics use Unreliable Multicast or OnRep.
- [ ] `ue_build` passes; PIE **Play As Client**, 2+ players, with network emulation (Bad): behavior matches on all windows.
- [ ] Late join: change state, then join/make relevant a new client - it sees current state.
- [ ] `ue_log` filtered on `LogNet|Error` clean after the session.
- [ ] If sessions/Steam/dedicated server: tested in Standalone or packaged, and the human was told what could not be tested in PIE.
