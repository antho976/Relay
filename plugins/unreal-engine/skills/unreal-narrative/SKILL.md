---
name: unreal-narrative
description: Narrative design in Unreal Engine 5 - story structure, environmental storytelling, character bibles, branching dialogue, quests, barks, game writing and subtitle limits, localization-ready text, data-driven dialogue, quests and story flags, cinematics from gameplay, voice lines. Use when writing or structuring story, or building dialogue, quest, story-flag, cutscene, subtitle or localization systems.
---

# Narrative Design and Implementation in Unreal Engine

Story in a game is authored *data* plus a *state* the game remembers. You will write text and
structure, and then implement it so that the player-facing words live in localizable `FText`, the
branching and progress live in Gameplay-Tag story flags, and everything is saved.

Reference files. Read each one when its topic comes up:
- `reference/dialogue-system.md`: read it before you build or extend a dialogue or bark system (Data Asset nodes, runner subsystem, choices, conditions, voice, UI hookup, validation).
- `reference/quest-system.md`: read it before you design or implement quests, objectives, quest logs or fail states.
- `reference/writing-guide.md`: read it before you write lines, barks, subtitles, item text or a character bible, or review a script for game-readiness and localization.

Related skills: `unreal-game-design` (pillars, loops, pacing), `unreal-level-environment`
(environmental storytelling spaces, Data Layers for world states), `unreal-audio` (VO playback,
concurrency, ducking), `unreal-gameplay-framework` (subsystems, game instance, save flow),
`unreal-ui-umg` (dialogue, subtitle and journal widgets), `unreal-animation` (cinematic and dialogue
animation), `unreal-cpp`, `unreal-blueprints`, and `unreal-multiplayer` (where story state is authoritative).

## 1. Story structure, adapted for play

- **Three-act**: setup (teach the verbs and the world, with the inciting incident within the first
  10–15 minutes of play), confrontation (escalating mechanics mirror escalating stakes, with a midpoint twist
  that recontextualizes the goal), resolution (a final test of every mechanic, then a short denouement).
- **Hero's journey**: useful as a checklist for a protagonist's arc (call, refusal, mentor, threshold,
  trials, ordeal, reward, return). Don't force all the stages. Map *threshold* moments to new
  regions or abilities.
- **Kishotenketsu** (introduction, development, twist, reconciliation): structure without
  central conflict. It suits cozy, puzzle and exploration games, and it also works *per level*: introduce a
  mechanic, develop it, twist it, then combine it.
- **Games-specific rules**: story beats land at gameplay *release* points (after a fight, on
  arrival), never in the middle of a skill test. The player's goal and the character's goal should align at the
  moment of play. Every cutscene should end by handing the player a clear objective.
- **Structure the content as data**: acts become chapters (a `Story.Chapter.N` flag), beats become quests or
  quest stages, and branches become flags (§5).

## 2. Environmental storytelling

- Tell by *arrangement*: a barricaded door with scratch marks, two coffee cups by a hastily shut
  laptop. Every story vignette should answer "what happened here?" in one glance.
- Layer it: the silhouette or landmark (seen from afar), then the composition of the room, then readable props
  (notes, audio logs, graffiti). Players skip the third layer, so it must be optional for comprehension.
- Put critical story on the critical path. Put lore off the path, as a reward for exploration.
- World state changes (the burned village) are best done as **World Partition Runtime Data Layers**
  switched by story flags, or as sub-levels streamed in. See `unreal-level-environment`.
- Readables and audio logs: the text lives in a Data Table or String Table (`FText`), is placed through an
  interactable actor that references the row, and is marked as "read" via a story flag (`Lore.Note.Harbor01`).

## 3. Character bibles

Write one per speaking character (the template is in `reference/writing-guide.md`): role in the story and in play,
want and need, voice (vocabulary, rhythm, verbal tics, what they'd *never* say), relationships, arc per
chapter, and a VO casting note. In data, give each character a Gameplay Tag (`Character.Mira`) and a
`UCharacterInfo` Data Asset (display name `FText`, portrait soft reference, subtitle color, voice). Dialogue
references the tag, never a raw string name.

## 4. Branching dialogue design

- **Hub and spoke**: a hub node with topic choices that return to the hub. Exhausted topics hide or grey out
  (track them with flags like `Dialogue.Mira.AskedAboutBell`).
- **Gates**: choices that require flags, stats or items. Show locked options greyed out when knowing
  they exist is interesting ("[Persuade] ..."), and hide them otherwise.
- **State**: conversations read and write story flags. Use *entry conditions* (first meeting vs.
  later visits, before and after a quest) instead of one giant tree.
- **Fold back**: branches rejoin quickly. Keep true divergence for a few high-value decisions,
  tracked by flags and paid off later. That scales far better than exponential trees.
- **Consequence visibility**: acknowledge choices soon, with a line, a bark, or a world change.
- **Player agency without paralysis**: 2–4 choices, each distinct in intent. Put the tone or intent in the choice
  text, not the exact line (or show the full line if the protagonist is silent).

## 5. Implementing story state (C++)

Story flags are **Gameplay Tags**, and counters are tag-keyed integers. One `UGameInstanceSubsystem`
owns them, survives level loads, and saves. It also relays story events that quests listen to.

Tag conventions (define them in `Config/Tags/Story.ini`, or `DefaultGameplayTags.ini`, or natively):
`Story.Chapter.2`, `Story.Choice.SparedWarden`, `Dialogue.Mira.Met`, `Quest.Harbor.Complete`,
`Event.Kill.Wolf`, `Event.Item.Pickup.Bell`, `Character.Mira`. Never rename a shipped tag. If you
must, add a redirect in `DefaultGameplayTags.ini` under `[/Script/GameplayTags.GameplayTagsSettings]` with
`+GameplayTagRedirects=(OldTagName="Old.Tag",NewTagName="New.Tag")`.

```cpp
// StoryStateSubsystem.h   Build.cs: "Core","CoreUObject","Engine","GameplayTags"
#pragma once
#include "CoreMinimal.h"
#include "Subsystems/GameInstanceSubsystem.h"
#include "GameFramework/SaveGame.h"
#include "GameplayTagContainer.h"
#include "StoryStateSubsystem.generated.h"

USTRUCT(BlueprintType)
struct FStoryCondition
{
	GENERATED_BODY()
	/** Every one of these flags must be set. Empty = no requirement. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Story") FGameplayTagContainer RequiredFlags;
	/** None of these flags may be set. */
	UPROPERTY(EditAnywhere, BlueprintReadWrite, Category = "Story") FGameplayTagContainer BlockedFlags;
};

UCLASS()
class MYGAME_API UStorySaveGame : public USaveGame
{
	GENERATED_BODY()
public:
	UPROPERTY() int32 Version = 1;
	UPROPERTY() FGameplayTagContainer Flags;
	UPROPERTY() TMap<FGameplayTag, int32> Counters;
};

DECLARE_DYNAMIC_MULTICAST_DELEGATE_TwoParams(FOnStoryFlagChanged, FGameplayTag, Flag, bool, bIsSet);
DECLARE_DYNAMIC_MULTICAST_DELEGATE_TwoParams(FOnStoryEvent, FGameplayTag, Event, int32, Count);
DECLARE_DYNAMIC_MULTICAST_DELEGATE(FOnStoryReloaded);

UCLASS()
class MYGAME_API UStoryStateSubsystem : public UGameInstanceSubsystem
{
	GENERATED_BODY()
public:
	UFUNCTION(BlueprintCallable, Category = "Story") void SetFlag(FGameplayTag Flag);
	UFUNCTION(BlueprintCallable, Category = "Story") void ClearFlag(FGameplayTag Flag);
	UFUNCTION(BlueprintCallable, Category = "Story") void ApplyFlags(const FGameplayTagContainer& FlagsToSet);
	UFUNCTION(BlueprintPure, Category = "Story") bool HasFlag(FGameplayTag Flag) const { return Flags.HasTagExact(Flag); }
	UFUNCTION(BlueprintPure, Category = "Story") bool MeetsCondition(const FStoryCondition& Condition) const;

	UFUNCTION(BlueprintCallable, Category = "Story") int32 AddToCounter(FGameplayTag Counter, int32 Delta = 1);
	UFUNCTION(BlueprintPure, Category = "Story") int32 GetCounter(FGameplayTag Counter) const;

	/** Something story-relevant happened (kill, pickup, dialogue beat). Quests listen to OnStoryEvent. */
	UFUNCTION(BlueprintCallable, Category = "Story") void SendStoryEvent(FGameplayTag Event, int32 Count = 1);

	UFUNCTION(BlueprintCallable, Category = "Story|Save") bool SaveToSlot(const FString& SlotName, int32 UserIndex = 0);
	UFUNCTION(BlueprintCallable, Category = "Story|Save") bool LoadFromSlot(const FString& SlotName, int32 UserIndex = 0);
	UFUNCTION(BlueprintCallable, Category = "Story") void ResetStory();

	/** For games with one combined save object: copy state in and out of it. */
	void WriteTo(UStorySaveGame& Save) const;
	void ReadFrom(const UStorySaveGame& Save);

	UPROPERTY(BlueprintAssignable, Category = "Story") FOnStoryFlagChanged OnFlagChanged;
	UPROPERTY(BlueprintAssignable, Category = "Story") FOnStoryEvent OnStoryEvent;
	UPROPERTY(BlueprintAssignable, Category = "Story") FOnStoryReloaded OnStoryReloaded;

private:
	UPROPERTY() FGameplayTagContainer Flags;
	UPROPERTY() TMap<FGameplayTag, int32> Counters;
};
```

```cpp
// StoryStateSubsystem.cpp
#include "StoryStateSubsystem.h"
#include "Kismet/GameplayStatics.h"

void UStoryStateSubsystem::SetFlag(FGameplayTag Flag)
{
	if (Flag.IsValid() && !Flags.HasTagExact(Flag))
	{
		Flags.AddTag(Flag);
		OnFlagChanged.Broadcast(Flag, true);
	}
}

void UStoryStateSubsystem::ClearFlag(FGameplayTag Flag)
{
	if (Flags.RemoveTag(Flag)) { OnFlagChanged.Broadcast(Flag, false); }
}

void UStoryStateSubsystem::ApplyFlags(const FGameplayTagContainer& FlagsToSet)
{
	for (const FGameplayTag& Tag : FlagsToSet) { SetFlag(Tag); }
}

bool UStoryStateSubsystem::MeetsCondition(const FStoryCondition& Condition) const
{
	// HasAllExact(empty) == true, HasAnyExact(empty) == false, so empty conditions pass.
	return Flags.HasAllExact(Condition.RequiredFlags) && !Flags.HasAnyExact(Condition.BlockedFlags);
}

int32 UStoryStateSubsystem::AddToCounter(FGameplayTag Counter, int32 Delta)
{
	int32& Value = Counters.FindOrAdd(Counter);
	Value += Delta;
	return Value;
}

int32 UStoryStateSubsystem::GetCounter(FGameplayTag Counter) const
{
	const int32* Value = Counters.Find(Counter);
	return Value ? *Value : 0;
}

void UStoryStateSubsystem::SendStoryEvent(FGameplayTag Event, int32 Count)
{
	if (Event.IsValid()) { OnStoryEvent.Broadcast(Event, Count); }
}

void UStoryStateSubsystem::WriteTo(UStorySaveGame& Save) const
{
	Save.Flags = Flags;
	Save.Counters = Counters;
}

void UStoryStateSubsystem::ReadFrom(const UStorySaveGame& Save)
{
	Flags = Save.Flags;
	Counters = Save.Counters;
	OnStoryReloaded.Broadcast();
}

bool UStoryStateSubsystem::SaveToSlot(const FString& SlotName, int32 UserIndex)
{
	UStorySaveGame* Save = Cast<UStorySaveGame>(UGameplayStatics::CreateSaveGameObject(UStorySaveGame::StaticClass()));
	if (!Save) { return false; }
	WriteTo(*Save);
	return UGameplayStatics::SaveGameToSlot(Save, SlotName, UserIndex);
}

bool UStoryStateSubsystem::LoadFromSlot(const FString& SlotName, int32 UserIndex)
{
	if (!UGameplayStatics::DoesSaveGameExist(SlotName, UserIndex)) { return false; }
	if (const UStorySaveGame* Save = Cast<UStorySaveGame>(UGameplayStatics::LoadGameFromSlot(SlotName, UserIndex)))
	{
		ReadFrom(*Save);
		return true;
	}
	return false;
}

void UStoryStateSubsystem::ResetStory()
{
	Flags.Reset();
	Counters.Reset();
	OnStoryReloaded.Broadcast();
}
```

Notes:
- `HasTagExact` is deliberate. With `HasTag`, a set flag `Story.Choice.SparedWarden` would make a
  check for `Story.Choice` true. Use hierarchical matching only when you intend it.
- For a large save, use `UGameplayStatics::AsyncSaveGameToSlot` to avoid hitches.
- If a flag tag is later removed from the tag list, it fails to load (with a warning in the log) and
  the flag is lost. That is another reason to never delete shipped story tags.
- In multiplayer, story state is server-authoritative. Replicate what clients need through the GameState
  (see `unreal-multiplayer`). This subsystem is per game instance, so it is local.
- From Blueprint: *Get Game Instance Subsystem (StoryStateSubsystem)*, then Set Flag / Has Flag.

## 6. Dialogue and quest data in UE

| Content | Asset | Why |
|---|---|---|
| Branching conversation | `UDialogueAsset : UPrimaryDataAsset` holding node structs | Branches, conditions and choices in one asset. See `reference/dialogue-system.md` |
| Bulk linear lines, barks, readables | `UDataTable` with a `FTableRowBase` row struct | Spreadsheet authoring, CSV round-trip, one row per line (the `RowName` is the line ID) |
| Quests | `UQuestAsset : UPrimaryDataAsset` + `UQuestSubsystem` | Stages, objectives and event matching. See `reference/quest-system.md` |
| Characters | `UCharacterInfo : UPrimaryDataAsset` keyed by a `Character.*` tag | Display name, portrait, subtitle color |
| All player-facing text | `FText` (String Tables for shared or UI text) | Localization |

Bulk lines and barks use a row struct derived from `FTableRowBase` (`FBarkRow`: Speaker tag, Context
tag, `FText` Line, soft Voice reference, `FStoryCondition`, Weight, Cooldown). It is defined in `reference/dialogue-system.md`.
The bark selection logic is there too. The CSV import pipeline (row struct path,
`CSVImportFactory`) is the same as in `unreal-game-design` `reference/balancing.md`.

## 7. Localization-ready text

- **All player-facing text is `FText`.** Never use `FString` or `FName` for display, and never build sentences by
  concatenation. Use `FText::Format` with named arguments:

```cpp
#define LOCTEXT_NAMESPACE "Quests"
const FText Title = LOCTEXT("Harbor_Title", "The Drowned Bell");
FFormatNamedArguments Args;
Args.Add(TEXT("Count"), FText::AsNumber(Remaining));
const FText Obj = FText::Format(LOCTEXT("Harbor_Collect",
	"Collect {Count} {Count}|plural(one=bell rope,other=bell ropes)"), Args);
#undef LOCTEXT_NAMESPACE

// Outside a LOCTEXT_NAMESPACE block:
const FText Pause = NSLOCTEXT("UI", "Pause_Title", "Paused");
```

- **String Tables** hold shared or UI strings with stable keys. Create one as an asset (Content Browser > Add >
  Miscellaneous > String Table), or register a CSV from C++ at startup:
  `LOCTABLE_FROMFILE_GAME("UI_ST", "UI", "Localization/StringTables/UI.csv");` (the path is relative to the
  project Content directory, and the CSV columns are `Key,SourceString,Comment`). Reference entries with
  `LOCTABLE("UI_ST", "Pause_Title")` in C++, `FText::FromStringTable(TableId, Key)`, or the *String Table*
  picker on any FText property. In CSV-imported Data Tables, an FText cell may contain
  `LOCTABLE("/Game/Localization/ST_Dialogue.ST_Dialogue","Mira_001")` to reference an entry.
- **Localization Dashboard** (Tools > Localization Dashboard): on the *Game* target, enable gathering from
  text files (`Source`) and from packages (`Content`), add the cultures, then Gather Text > Export Text (PO) >
  translate > Import Text > Compile Text. The output goes to `Content/Localization/Game/<culture>/`. Add cultures
  to Project Settings > Packaging > *Localizations to Package*. The dashboard config is text in
  `Config/Localization/*.ini`.
- **Preview and switch**: Editor Preferences > Region & Language > *Preview Game Language*. At runtime use
  `UKismetInternationalizationLibrary::SetCurrentCulture(TEXT("fr"), /*SaveToConfig*/ true)`, or pass `-culture=fr`
  on the command line.
- Write for expansion: German and French run about 30% longer than English, so UI must wrap or scale. No text inside
  textures. See `reference/writing-guide.md` for writer-side rules.

## 8. Quest design (summary)

A quest is a state machine: **Inactive → Active (stage 0..N) → Completed | Failed**. Stages contain
objectives, which are satisfied by story events (`Event.Kill.Wolf` ×5) or flags. Design rules:
- One clear verb-based objective line at a time ("Ring the harbor bell"). Optional objectives are marked as such.
- Every objective needs a *where* (a marker, a landmark, or dialogue direction) and a *done* signal (a line, a UI update, a sound).
- Prefer **fail-forward** (a branch in the story) over hard failure. Hard fails need a clear cause and a quick retry.
- Quest givers acknowledge state changes (entry conditions on their dialogue).
The data layout, runtime subsystem, event matching and save are in `reference/quest-system.md`.

## 9. Barks and systemic dialogue

Barks are short contextual lines (combat callouts, idle chatter, reactions). Rules: every bark
has a context tag, a speaker, a priority, a cooldown per line and per speaker, and a global
concurrency cap, and it never repeats the same line back to back. Story flags make barks
reactive ("the guard mentions the burned mill"). Keep barks at 2–6 words for combat, and at most one sentence otherwise.
The implementation sketch is in `reference/dialogue-system.md`.

## 10. Cinematics with Sequencer

- **Level Sequence** assets hold camera cuts, actor bindings, animation, audio, events and fades. Use Cine
  Camera Actors and a **Camera Cut track** to switch cameras.
- **Bindings**: *possessables* reference level actors, and *spawnables* are owned by the sequence. To animate the
  player or another runtime actor, tag the binding (right-click the binding > Tags, for example `Player`) and override
  it at runtime with `ALevelSequenceActor::SetBindingByTag`.
- **Gameplay hooks**: an **Event Track** fires Sequence Director Blueprint events at keys (enable input, set a
  story flag, start a quest). End the sequence by handing control back with an objective.
- **Triggering from gameplay** (C++):

```cpp
// Build.cs: "LevelSequence", "MovieScene"
#include "LevelSequence.h"
#include "LevelSequenceActor.h"
#include "LevelSequencePlayer.h"
#include "MovieSceneSequencePlaybackSettings.h"

// Header: UPROPERTY(EditAnywhere) TObjectPtr<ULevelSequence> Sequence;
//         UPROPERTY() TObjectPtr<ALevelSequenceActor> SequenceActor;
//         UFUNCTION() void HandleSequenceFinished();

void AStoryTrigger::PlayCinematic(APawn* PlayerPawn)
{
	FMovieSceneSequencePlaybackSettings Settings;
	Settings.bDisableMovementInput = true;
	Settings.bDisableLookAtInput = true;
	Settings.bHidePlayer = false;          // true if the sequence uses its own spawnable double
	Settings.bHideHud = true;

	ALevelSequenceActor* OutActor = nullptr;
	ULevelSequencePlayer* Player = ULevelSequencePlayer::CreateLevelSequencePlayer(GetWorld(), Sequence, Settings, OutActor);
	if (!Player || !OutActor) { return; }
	SequenceActor = OutActor;
	SequenceActor->SetBindingByTag(TEXT("Player"), { PlayerPawn });
	Player->OnFinished.AddDynamic(this, &AStoryTrigger::HandleSequenceFinished);
	Player->Play();
}
// Skip button: SequenceActor->GetSequencePlayer()->GoToEndAndStop();
```

  In Blueprint: *Create Level Sequence Player*, then *Play*, then bind *On Finished*. Or place a Level Sequence
  Actor in the level and call Play on its player.
- Make cutscenes skippable (hold to skip), set flags in `HandleSequenceFinished` (so a skip also sets
  them), and put subtitles in the sequence (see §11).

## 11. Delivering lines: audio and subtitles

- The VO pipeline: `Content/Audio/VO/<culture?>/<Character>/<LineID>.wav`, imported as Sound Waves (or
  **Dialogue Waves**, which pair a `UDialogueVoice` speaker with spoken text and support localized VO and context
  variants; play them with `UGameplayStatics::PlayDialogue2D` or `SpawnDialogueAttached`).
- Route VO to a Dialogue Sound Class and submix. Duck music and SFX under VO with a Sound Mix or submix
  sidechain, and limit concurrent VO per speaker with Sound Concurrency. See `unreal-audio`.
- **Subtitles**: most games render them in UMG from the dialogue system (speaker name, color, text,
  background box), timed by the voice duration (`USoundBase::GetDuration()`) or split into timed cues.
  The engine also has built-in subtitle cues on Sound Waves, toggled by `UGameplayStatics::SetSubtitlesEnabled`.
  They are basic, so use a custom widget for a shipped game.
- Subtitle rules (in detail in `reference/writing-guide.md`): about 42 characters per line or fewer, at most 2 lines, speaker
  name shown, on by default, with size and background options.
- Lines without VO yet: use a placeholder duration of `max(1.5 s, characters / 15 per second)`. Temporary TTS files
  are fine, but mark them in the file name (`_TEMP`).

## 12. Common pitfalls

- Player-facing strings in `FString`, `FName`, `PrintString`, or text baked into textures, which can't be localized.
- `FText::FromString` for authored text. It creates text with no localization key, so the gatherer never collects it. Use LOCTEXT, NSLOCTEXT or string tables, and keep `FromString` for runtime data such as player names.
- Story logic scattered across level Blueprints. Keep flags and quest logic in the subsystems, and have the level only *send events*.
- Setting flags at the start of a cutscene instead of at its end (or in the skip path), so skipping breaks progression.
- Renaming or deleting shipped Gameplay Tags, which silently breaks saves.
- Branching every choice into unique content, which makes scope explode. Fold back.
- Long unskippable exposition during gameplay tension.

## 13. Verify your work

- [ ] Every new player-facing string is `FText` via LOCTEXT, NSLOCTEXT or a String Table. A Localization Dashboard gather picks it up (no "text not gathered" warnings).
- [ ] New story tags are registered (ini or native), and `ue_log` shows no "Requested Gameplay Tag … was not found" errors.
- [ ] Dialogue and quest assets pass Data Validation (right-click > Asset Actions > Validate), with no dangling node IDs.
- [ ] Save → quit → load restores flags, counters and quest states (test in a Standalone game, not just PIE).
- [ ] Cutscenes: skip works and still sets flags, input is restored, and the HUD comes back.
- [ ] Subtitles appear for every VO line, within the line and character limits, with the speaker named.
- [ ] C++ builds (`ue_build`), Build.cs has `GameplayTags` (and `LevelSequence`, `MovieScene`, `UMG` if used), and you saved the changed assets and listed them for the human.
