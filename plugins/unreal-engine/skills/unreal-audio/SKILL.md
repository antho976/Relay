---
name: unreal-audio
description: Unreal Engine 5 audio - Sound Waves and Cues, MetaSounds, playing sounds from C++, Blueprint and notifies, attenuation and occlusion, concurrency, classes, mixes and submixes, Audio Modulation, music with Quartz, dialogue and subtitles, budgets. Use for any SFX, music, footsteps, UI sound, volume setting, ducking or audio bug.
---

# Unreal Audio

UE5 uses the Audio Mixer (software mixing on all platforms). Sound assets are binary: import
and configure them through `ue_python` or give the human click-paths; C++ and Blueprint
only trigger and parameterize them at runtime.

Related: `unreal-gas` (Gameplay Cues for ability sounds), `unreal-multiplayer` (who should
hear what), `unreal-materials-vfx` (pair VFX with sound), `unreal-animation` (notifies),
`unreal-ui-umg` (subtitle and settings widgets), `unreal-blueprints`.

## Asset types and when to use each

| Asset | Use for |
|---|---|
| **Sound Wave** | Imported audio file (.wav preferred; newer 5.x versions also import formats such as .ogg/.flac/.aiff). The raw data plus compression, loading and subtitle settings. |
| **Sound Cue** | Legacy node graph: random variation, pitch/volume modulation, looping, switch by parameter. Still fully supported and simple. |
| **MetaSound Source** | Procedural, sample-accurate audio graph (5.0+). Preferred for new work: variations, layered/interactive sounds, engines, music stems, synthesis. |
| **MetaSound Patch** | Reusable sub-graph used inside MetaSound Sources (no playback by itself). |
| **Sound Attenuation** | Shared distance/spatialization settings. |
| **Sound Concurrency** | Voice limiting rules for a group of sounds. |
| **Sound Class** | Category hierarchy (Master > SFX/Music/Dialogue/UI) for volume/pitch control and properties. |
| **Sound Mix** | Legacy snapshot that adjusts Sound Classes (ducking, pause menu). |
| **Sound Submix** | DSP bus graph: effects (reverb, EQ, compression) applied to the summed audio of routed sounds. |
| **Control Bus / Control Bus Mix** | Audio Modulation plugin: parameter buses for volume sliders, ducking, dynamic mixing. |
| **Dialogue Voice / Dialogue Wave** | Localizable dialogue with speaker/listener context and subtitles. |

Decision: new projects use MetaSounds for anything with logic, Sound Waves directly for
simple one-offs, Audio Modulation for the user-facing volume settings and mix states. Keep
Sound Cues if the project already uses them - do not migrate for its own sake.

## Importing and configuring sound waves

- Source audio: 48 kHz (or 44.1 kHz), 16- or 24-bit WAV, mono for anything spatialized
  (footsteps, impacts, voices in the world), stereo for music/ambient beds/UI.
- Import via drag-and-drop, or Python:
  ```python
  import unreal
  task = unreal.AssetImportTask()
  task.set_editor_property("filename", "D:/Audio/SFX/Footstep_Grass_01.wav")
  task.set_editor_property("destination_path", "/Game/Audio/SFX/Footsteps")
  task.set_editor_property("automated", True)
  task.set_editor_property("replace_existing", True)
  task.set_editor_property("save", True)
  unreal.AssetToolsHelpers.get_asset_tools().import_asset_tasks([task])
  print(task.get_editor_property("imported_object_paths"))
  ```
- Batch settings (sound class, attenuation, looping):
  ```python
  import unreal
  sfx_class = unreal.load_asset("/Game/Audio/Classes/SC_SFX")
  att = unreal.load_asset("/Game/Audio/Attenuation/ATT_Footsteps")
  with unreal.ScopedEditorTransaction("Configure footstep waves"):
      for p in unreal.EditorAssetLibrary.list_assets("/Game/Audio/SFX/Footsteps", recursive=True):
          w = unreal.load_asset(p)
          if isinstance(w, unreal.SoundWave):
              w.set_editor_property("sound_class_object", sfx_class)
              w.set_editor_property("attenuation_settings", att)
              w.set_editor_property("looping", False)
              unreal.EditorAssetLibrary.save_asset(p)
  ```
  If a property name fails, list them with `dir(w)` / `help(unreal.SoundWave)` rather than guessing.
- Naming: follow the project's existing convention. This skill uses `SC_` for Sound Classes,
  `SCue_` for cues, `MSS_`/`MSP_` for MetaSound Sources/Patches, `ATT_` attenuation, `CON_`
  concurrency, `MIX_` sound mixes, `CB_`/`CBM_` control buses/mixes.

## Playing sounds

C++ (`#include "Kismet/GameplayStatics.h"`, `#include "Components/AudioComponent.h"`; no extra
Build.cs module needed; hold assets as `UPROPERTY(EditDefaultsOnly) TObjectPtr<USoundBase>` -
`USoundBase` accepts Waves, Cues and MetaSound Sources):

```cpp
// Fire-and-forget 3D one-shot
UGameplayStatics::PlaySoundAtLocation(this, ImpactSound, Hit.ImpactPoint, FRotator::ZeroRotator,
    /*Volume*/ 1.f, /*Pitch*/ 1.f, /*StartTime*/ 0.f, ImpactAttenuation, ImpactConcurrency);

// 2D (UI, stingers, local-only feedback)
UGameplayStatics::PlaySound2D(this, UIClickSound);

// Need control afterwards (stop, fade, parameters): get a component
UAudioComponent* AC = UGameplayStatics::SpawnSoundAttached(EngineLoop, GetMesh(), TEXT("Exhaust"));
if (AC) { AC->SetFloatParameter(TEXT("RPM"), Rpm); }
```

Persistent component (engine loops, ambient emitters, music):
```cpp
// Header
UPROPERTY(VisibleAnywhere, BlueprintReadOnly, Category = "Audio")
TObjectPtr<UAudioComponent> EngineAudio;

// Constructor
EngineAudio = CreateDefaultSubobject<UAudioComponent>(TEXT("EngineAudio"));
EngineAudio->SetupAttachment(GetRootComponent());
EngineAudio->bAutoActivate = false;

// Runtime
EngineAudio->SetSound(EngineMetaSound);
EngineAudio->SetFloatParameter(TEXT("RPM"), 900.f);   // set before Play to affect the start
EngineAudio->Play();
EngineAudio->FadeOut(0.5f, 0.f);                     // fade then stop
EngineAudio->FadeIn(1.0f);
EngineAudio->SetVolumeMultiplier(0.8f);
EngineAudio->OnAudioFinished.AddDynamic(this, &AMyActor::HandleAudioFinished); // UFUNCTION() void HandleAudioFinished();
```
Parameters on an audio component (work for MetaSound inputs and Sound Cue parameter nodes):
`SetFloatParameter`, `SetIntParameter`, `SetBoolParameter`, `SetTriggerParameter` (MetaSound
trigger inputs), `SetWaveParameter`, `SetObjectParameter`. Names are `FName`s and must
match the MetaSound input name exactly; a wrong name is silently ignored.

Parameters on a fire-and-forget sound: pass `UInitialActiveSoundParams` (the last argument
of `PlaySoundAtLocation`/`PlaySound2D`), filled with `FAudioParameter` entries, e.g.
`Params->AudioParams.Add(FAudioParameter(TEXT("Surface"), 2));`. Check the constructor
overloads in `AudioParameter.h` if the type is ambiguous. Otherwise use a component.

Blueprint: "Play Sound at Location", "Play Sound 2D", "Spawn Sound Attached", "Spawn Sound at
Location", then "Set Float Parameter", "Execute Trigger Parameter" on the returned component.

Networking: sounds are cosmetic and local. The acting player hears their own action
immediately (play locally on input), others hear it via unreliable multicast or an OnRep.
Dedicated servers have no audio device; skip sound work there.

## Sound Cues (legacy graph)

Typical one-shot variation cue: `Wave Player` x N -> `Random` (check "Randomize Without
Replacement") -> `Modulator` (Pitch 0.95-1.05, Volume 0.9-1.0) -> Output. Other nodes:
`Looping`, `Concatenator`, `Mixer`, `Delay`, `Branch` (bool param), `Switch` (int param),
`Crossfade by Param`, `Attenuation`, `Continuous Modulator` (param-driven volume/pitch).
Creating cue graphs is a human task in the editor; the agent can assign cues and set parameters.

## MetaSounds

Create: Content Browser > Add > Audio > MetaSound Source (or MetaSound Patch). Key concepts:
- Graph inputs = parameters settable at runtime (Float, Int32, Bool, Trigger, Wave Asset,
  Time, arrays). Outputs are audio channels (mono/stereo chosen in Source settings) plus
  interface outputs.
- Interfaces: every Source has `On Play` (trigger). `UE.Source.OneShot` adds an `On Finished`
  output - **a one-shot Source must trigger On Finished**, or it plays (silently) until
  stopped and keeps its voice. Sources without OneShot are treated as looping/continuous.
  `UE.Attenuation` exposes listener distance; `UE.Spatialization` exposes azimuth/elevation.
- Common nodes: `Wave Player` (mono/stereo, loop, start time, pitch shift), `Random Get`
  (pick a wave from an array input, no-repeat option), `Trigger Repeat`, `Trigger Delay`,
  `Trigger Sequence`, `Trigger Route`, `AD Envelope`/`ADSR Envelope`, `Mixer` (mono/stereo),
  filters (`Biquad Filter`, `One-Pole`, `State Variable Filter`), `Delay`, `Map Range`,
  `Interp To`, oscillators and noise for synthesis.
- Presets: right-click a Source > "Create MetaSound Preset" to reuse a graph with different
  input defaults (e.g. one "Footstep" graph, per-surface presets).
- Patches: shared logic (e.g. a "randomized one-shot" patch) referenced as a node.
- Graph authoring requires the MetaSound editor (human task). The MetaSound Builder API
  (runtime/editor graph construction from Blueprint/Python) exists in newer 5.x releases; it
  is not the default path - verify availability with `help(unreal.MetaSoundEditorSubsystem)`
  / engine docs before relying on it.
- Referencing the `UMetaSoundSource` type in C++ needs `MetasoundEngine` in Build.cs; prefer
  `USoundBase*` properties and avoid the dependency.

## Attenuation and spatialization

Create a Sound Attenuation asset per category (footsteps, weapons, ambience, voices) and
share it; per-sound overrides are hard to maintain.
- **Attenuate (Volume)**: Shape (Sphere, Capsule, Box, Cone), Inner Radius (full volume),
  Falloff Distance, Attenuation Function (Linear, Logarithmic, Inverse, Log Reverse, Natural
  Sound, Custom curve). Natural Sound is a good default for realistic falloff.
- **Spatialize**: Panning (default, cheap) or Binaural (HRTF; requires a spatialization
  plugin selected in Project Settings > Platforms > <Platform> > Audio). Non-Spatialized
  Radius blends to 2D near the listener; Stereo Spread for multichannel sources.
- **Air absorption** (Attenuate with Low Pass Filter): distance-based LPF - sells distance
  more than volume alone.
- **Occlusion**: line trace per sound on a channel, applies LPF/volume when blocked. Costs a
  trace per playing sound; enable only for important sounds (enemy footsteps, gunfire).
- **Reverb Send**: per-distance send amount to the reverb submix.
- **Focus**: boost sounds in front of the listener, attenuate behind; good for shooters.
- **Priority attenuation**: lowers priority with distance so voice stealing picks far sounds.
- Listener is the player camera by default; for third-person games consider
  `PlayerController->SetAudioListenerOverride(Component, Location, Rotation)` (or the
  attenuation override position) so distances are measured from the character.
Debug in PIE: `au.3dVisualize.Enabled 1` draws active sounds; `stat SoundWaves`,
`stat SoundCues`, `stat Sounds` list what is playing.

## Concurrency (voice limiting)

Sound Concurrency asset: **Max Count**, **Limit to Owner** (per actor), **Resolution Rule**
(Prevent New, Stop Oldest, Stop Farthest Then Prevent New, Stop Farthest Then Oldest, Stop
Lowest Priority, Stop Quietest, Stop Lowest Priority Then Prevent New), **Retrigger Time**
(min seconds between starts), **Volume Scale** ducking of older voices.
Guidelines: footsteps per character Limit to Owner, max 2-3; impacts global max 6-8 with Stop
Farthest Then Oldest; UI clicks max 1-2 with Stop Oldest; weapon fire per owner.
The project default concurrency lives in Project Settings > Audio. The hardware voice limit
is **Max Channels** (Project Settings > Platforms > <Platform> > Audio); beyond it the
lowest-priority sounds are culled or virtualized.

## Sound Classes, Mixes and Submixes

- Sound Class hierarchy: `SC_Master` > `SC_Music`, `SC_SFX` (> Weapons, Footsteps, Ambience),
  `SC_Dialogue`, `SC_UI`. Assign every sound (via Details > Sound Class, or batch Python).
  Set the default in Project Settings > Audio > Default Sound Class.
- Legacy volume control: a Sound Mix with class adjusters, then
  ```cpp
  UGameplayStatics::SetSoundMixClassOverride(this, UserMix, MusicClass, Volume, 1.f, 0.f, true);
  UGameplayStatics::PushSoundMixModifier(this, UserMix);  // the override only applies while the mix is pushed
  ```
  Pause menu or ducking: push a mix, pop it with `PopSoundMixModifier`. Sound Classes can
  have Passive Sound Mix Modifiers (duck music whenever dialogue plays).
- Submixes: DSP routing graph, Master Submix at the root; project defaults (Master, Reverb,
  EQ, base default submix) in Project Settings > Audio. Route sounds with the Submix setting
  on the sound (or its Sound Class). Put effects on submixes, not on hundreds of sources:
  Submix Effect Presets such as Reverb, EQ, Dynamics Processor (compressor/limiter,
  sidechain ducking), Delay. A limiter on the master submix prevents clipping.
- Source Effect Chains apply per-voice effects (filters, bitcrush, etc.); more CPU per voice.

## Audio Modulation (recommended for volume settings and mix states)

Enable plugin **Audio Modulation**; Build.cs `AudioModulation` if used from C++.
- Control Bus (e.g. `CB_Music`, parameter Volume), Control Bus Mix (a set of bus values:
  `CBM_UserSettings`, `CBM_PauseMenu`, `CBM_Combat`), Modulation Generators (LFO, envelope
  follower), Modulation Patches (combine buses/curves).
- Attach buses in the Modulation section of sounds, Sound Classes or Submixes (e.g. every
  music sound or the Music submix gets Volume modulation from `CB_Music`).
- User volume sliders (C++):
  ```cpp
  #include "AudioModulationStatics.h"
  UAudioModulationStatics::ActivateBusMix(this, UserSettingsMix);          // once, e.g. at game start
  FSoundControlBusMixStage Stage = UAudioModulationStatics::CreateBusMixStage(this, MusicBus, SliderValue01);
  UAudioModulationStatics::UpdateMix(this, UserSettingsMix, { Stage });
  ```
  Save the slider values in your settings/save game and re-apply on startup.
- Mix states (combat, underwater, pause): `ActivateBusMix` / `DeactivateBusMix` with attack/
  release times set on the mix stages.

## Reverb and environment

- **Audio Volume** actor (brush): Reverb Settings (Reverb Effect asset, volume, fade time) and
  Interior Settings (exterior volume/LPF for sounds outside the volume - rooms feel enclosed).
  Priority decides which volume wins when nested.
- **Submix reverb**: a reverb submix with a Reverb submix effect, fed by per-sound reverb
  sends (attenuation Reverb Send). Convolution reverb is available through the Synthesis
  plugin's Convolution Reverb submix effect (needs an impulse response asset).
- Ambience: layered loops (bed + random one-shots via a MetaSound with Trigger Repeat and
  random delay/position), placed as Ambient Sound actors with attenuation, plus Audio Volumes.

## Music systems

- Put music on `SC_Music` / its own submix, non-spatialized, with its own concurrency (max 1-2
  per layer) and `bIsUISound = true` on the component if it must keep playing while paused.
- **Layered/stem music**: one MetaSound Source with N `Wave Player` nodes started by the same
  trigger (sample-accurate sync), each multiplied by a gain input (`Interp To` for smooth
  fades), summed in a Mixer. Drive `Intensity` (float) from gameplay; the graph maps it to
  stem gains. Keep all stems the same length and tempo.
- **Horizontal transitions** (switching sections): use Quartz so changes land on bar/beat
  boundaries.
- **Quartz** (sample-accurate clock in the audio renderer): Blueprint flow - Get Quartz
  Subsystem > Create New Clock (name, time signature) > Set Beats Per Minute > Start Clock;
  then on an Audio Component "Play Quantized" with a Quantization Boundary (Bar, Beat, etc.);
  "Subscribe to Quantization Event" on the clock handle to fire gameplay (lights, beat-synced
  VFX, rhythm input windows) exactly on the beat. The C++ API mirrors these nodes
  (`UQuartzSubsystem`, `UQuartzClockHandle`, `UAudioComponent::PlayQuantized`); read the
  headers in the project's engine version for exact signatures before writing C++.
- Stingers (one-shot musical accents): 2D, play quantized to the next beat.

## Dialogue and subtitles

- Simple: Sound Waves with subtitle entries (Sound Wave > Subtitles: time + text; also
  "Comment" and "Mature" flags). The engine's built-in subtitle display is basic; for a
  styled UMG subtitle widget, drive it from your dialogue system data or listen to the
  engine subtitle manager's text delegate (`FSubtitleManager` in `SubtitleManager.h` - verify
  the delegate name in your version).
- Localized/contextual: Dialogue Voice assets (speaker) + Dialogue Wave (spoken text, context
  mappings speaker -> listeners -> Sound Wave). Play with
  `UGameplayStatics::PlayDialogueAtLocation(this, DialogueWave, Context, Location)` or
  `PlayDialogue2D` / `SpawnDialogueAttached`, where `FDialogueContext` has `Speaker` and
  `Targets`. Dialogue Waves participate in localization gathering (per-culture audio).
- Dialogue gets its own Sound Class, concurrency (1 per speaker, Stop Oldest or Prevent New
  depending on priority), and ducks music/SFX via a passive mix or a bus mix.

## Sounds from animation

- Built-in notify: in the Animation Sequence/Montage editor, Notifies track > Add Notify >
  Play Sound; set Sound, "Follow" and "Attach Name" (socket) for moving sources.
- Surface-aware footsteps (custom notify):
  ```cpp
  UCLASS(meta = (DisplayName = "Footstep"))
  class MYGAME_API UAnimNotify_Footstep : public UAnimNotify
  {
      GENERATED_BODY()
  public:
      virtual void Notify(USkeletalMeshComponent* MeshComp, UAnimSequenceBase* Animation,
                          const FAnimNotifyEventReference& EventReference) override;
      UPROPERTY(EditAnywhere) FName FootSocket = TEXT("foot_l");
      UPROPERTY(EditAnywhere) TMap<TEnumAsByte<EPhysicalSurface>, TObjectPtr<USoundBase>> SurfaceSounds;
      UPROPERTY(EditAnywhere) TObjectPtr<USoundBase> DefaultSound;
  };
  ```
  ```cpp
  #include "Kismet/GameplayStatics.h"
  #include "PhysicalMaterials/PhysicalMaterial.h"   // Build.cs: PhysicsCore

  void UAnimNotify_Footstep::Notify(USkeletalMeshComponent* MeshComp, UAnimSequenceBase* Animation,
                                    const FAnimNotifyEventReference& EventReference)
  {
      Super::Notify(MeshComp, Animation, EventReference);
      UWorld* World = MeshComp ? MeshComp->GetWorld() : nullptr;
      if (!World || World->GetNetMode() == NM_DedicatedServer) { return; }

      const FVector Start = MeshComp->GetSocketLocation(FootSocket) + FVector(0, 0, 20);
      const FVector End = Start - FVector(0, 0, 60);
      FCollisionQueryParams Params(SCENE_QUERY_STAT(FootstepTrace), false, MeshComp->GetOwner());
      Params.bReturnPhysicalMaterial = true;
      FHitResult Hit;
      USoundBase* Sound = DefaultSound;
      if (World->LineTraceSingleByChannel(Hit, Start, End, ECC_Visibility, Params))
      {
          const EPhysicalSurface Surface = UPhysicalMaterial::DetermineSurfaceType(Hit.PhysMaterial.Get());
          if (const TObjectPtr<USoundBase>* Found = SurfaceSounds.Find(Surface)) { Sound = *Found; }
      }
      if (Sound) { UGameplayStatics::PlaySoundAtLocation(World, Sound, Hit.bBlockingHit ? Hit.ImpactPoint : Start); }
  }
  ```
  Surface types are defined in Project Settings > Physics > Physical Surface and assigned
  through Physical Materials on landscape layers/materials. Alternatively pass the surface as
  an Int parameter to one footstep MetaSound.
- Notify State for loops (whoosh during a swing): start on `NotifyBegin`, `FadeOut` on `NotifyEnd`.
- Montage/section-level sounds in GAS abilities: prefer Gameplay Cues (see `unreal-gas`).

## Budgets and optimization

- **Voices**: Max Channels per platform; concurrency everywhere; priorities (weapons/dialogue
  high, ambience low). **Virtualization Mode** on looping sounds: `Play When Silent` keeps
  tracking playback while inaudible/culled and resumes in place; `Restart` restarts the sound
  when it becomes audible again; `Disabled` stops it for good when culled.
- **Memory/loading**: stream caching is standard in UE5 (per-platform cache size in Project
  Settings > Platforms > <Platform> > Audio). Per wave "Loading Behavior Override": `Retain On
  Load` for latency-critical short sounds (weapons, footsteps, UI), `Prime On Load` for
  sounds needed soon, `Load On Demand` for long/rare audio (music, dialogue).
- **Compression**: Sound Wave > Compression Quality and Sound Asset Compression Type (options
  depend on version: ADPCM, PCM, Bink Audio, Opus, and RAD Audio in newer releases).
  ADPCM for short, frequently-triggered sounds (cheap to decode); perceptual codecs for music,
  dialogue and long ambience. Downsample where inaudible (per-platform Sample Rate overrides).
- **CPU**: effects on submixes not sources; limit occlusion and binaural to important sounds;
  MetaSounds cost per voice - keep one-shot graphs small; stop inaudible loops
  (attenuation + virtualization) rather than leaving hundreds playing at zero volume.
- Profiling: `stat audio`, `stat SoundWaves`, `stat SoundCues`, `stat Sounds`,
  `au.3dVisualize.Enabled 1`, and Unreal Insights (audio channels; the Audio Insights plugin in
  newer 5.x versions - check availability). Read `ue_log` filtered on `LogAudio|LogAudioMixer|LogMetaSound`.

## Sound design for game feel

- **Latency kills feel**: play the player's own action sounds locally on input (weapon fire,
  jump, UI), not after a server round trip. Keep attack/impact sounds Retain On Load.
- **Variation**: 3-8 takes per frequent sound, random without repeat, pitch +-3-5%, volume
  +-1-2 dB. Repetition is the fastest way to make a game sound cheap.
- **Layering**: transient (click/snap) + body + tail/reverb; distant variants for weapons
  (swap by distance or crossfade via attenuation/MetaSound Distance input).
- **Sync**: impacts on the exact frame (anim notifies, hit events); music-reactive gameplay via Quartz.
- **Readability and priority**: important information (enemy footsteps, reload, low health)
  must win the mix; duck ambience/music under dialogue and critical cues; limit simultaneous
  impacts so the mix does not turn to noise.
- **Consistency**: normalize source loudness per category before import; mix with Sound
  Classes/Buses, not by editing hundreds of per-asset volumes.
- **Space**: reverb and LPF sell environment and distance; silence and contrast make big
  moments land.
- **Accessibility**: separate volume sliders (Master, Music, SFX, Dialogue, UI), subtitles with
  speaker names, and visual cues for critical sounds.

## Common pitfalls

- Stereo file on a spatialized sound - it does not localize well; use mono for 3D.
- One-shot MetaSound without `On Finished` connected - voices never free.
- `SetSoundMixClassOverride` without `PushSoundMixModifier` - no effect.
- Playing sounds in Tick or on every overlap without concurrency - voice spam and clipping.
- Parameter name typos (MetaSound inputs, cue params) - silently ignored.
- Using `GetPlayerController(0)` for listener logic in split screen or on a listen server.
- Sounds played on the server only in multiplayer - clients hear nothing (see `unreal-multiplayer`).
- Music and SFX in the same Sound Class - volume sliders cannot separate them.

## Verify your work

- [ ] Every new sound has a Sound Class (and submix routing), attenuation (if 3D) and concurrency.
- [ ] 3D sources are mono; music/UI are 2D.
- [ ] Parameter names in code match MetaSound inputs / cue params exactly.
- [ ] PIE: `au.3dVisualize.Enabled 1` and `stat Sounds` show expected voices; nothing lingers after one-shots end.
- [ ] Volume sliders persist and affect the right categories.
- [ ] `ue_build` passes (Build.cs has `AudioModulation`/`PhysicsCore`/`MetasoundEngine` only if used); `ue_log` has no `LogAudio` errors.
- [ ] Human told which audio assets were created/changed and saved.
