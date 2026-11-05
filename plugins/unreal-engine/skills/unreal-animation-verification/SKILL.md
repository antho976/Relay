---
name: unreal-animation-verification
description: Verify and fix animation, rigging and attachments in Unreal Engine 5 for any skeleton, any held or worn item, and two-character interactions - axes, sockets and grips, one- and two-handed holds, IK, clipping, contact timing, retargeting mistakes. Use when an item sits wrong or floats, a character clips, an interaction does not line up or an animation feels off, and before reporting animation work as done. Uses ue_anim_inspect and ue_anim_preview.
---

# Animation verification, rigging and attachments

You cannot watch an animation. Every error in this area is spatial: the wrong hand, an item
rotated 90 degrees, a grip 10 cm off the palm, a blade through the torso, a hug where the arms
pass through the other character. Reading C++ or asset settings does not catch these. So this
skill has one hard rule:

> **Never report animation, rigging, attachment or interaction work as done until
> `ue_anim_inspect` passes and you have looked at `ue_anim_preview` images of it.**
> If you cannot run them (editor closed), say plainly that the result is unverified.

Load `unreal-animation` for how Animation Blueprints, montages and IK nodes work; this skill is
about checking that the result is physically right. `unreal-editor-automation` covers the
editor tools in general.

## 1. The verification loop

Run this for every change, and first on the existing state when you are asked to fix something
(measure before you touch anything):

1. **Identify the pieces.** Skeletal mesh, animation sequence(s), attached items and the bone or
   socket each hangs from, any partner character. Find paths with `ue_search_assets`
   (`class_names: ["SkeletalMesh"]`, `["AnimSequence"]`, `["StaticMesh"]`), sockets by reading the
   Skeleton / Skeletal Mesh with `ue_python` (see reference/inspect-recipes.md).
2. **Measure: `ue_anim_inspect`.** Describe the setup (attachments with grips, partner, the
   contacts you expect) and read `problems`, `attachments[*].side`, `closest_approach`.
3. **Look: `ue_anim_preview`.** Front and right views at 4-8 times. The preview spawns 500 m
   above the editor camera, clear of the level (`isolate` is off by default). Check each
   image for: which hand holds the item, which way the item points, whether hands wrap the grip,
   whether anything passes through a body, whether feet are on the ground, and the silhouette of
   the key pose.
4. **Fix one cause at a time** (section 8 maps symptoms to causes). Prefer data fixes (socket
   transform, attach rule, IK target, animation choice, timing) over re-authoring.
5. **Re-measure and re-look.** Same arguments, compare numbers. Stop when `passed` is true and the
   images show what the design asked for.
6. **Report with evidence:** the numbers that passed (grip distances, minimum clearances, sides),
   which images you checked, and anything that needs a human (new animation, rig change in a
   DCC tool).

**Runtime check.** `ue_anim_inspect` reads animation files; what the player sees also depends on
the Animation Blueprint's IK, attachment code and first-person camera setup. For first-person
hands and weapons, finish with `ue_play` using `outside` (for example `{"views": ["right",
"front"], "distance": 80}`) to look at the live arms and gun from outside the camera, and a
`probe` that prints the hand and weapon socket locations (`get_socket_location`) at the same
checkpoints.

Pass criteria to use unless the design says otherwise:

| Check | Pass |
|---|---|
| Held item side | `attachments[name].side` matches the design (e.g. `right` for a right-handed hold) |
| Grip | every grip within 3-5 cm of the palm at every sample (measure against a palm socket - hand bone origins sit at the wrist, 8-10 cm from the palm on most skeletons) |
| Item vs own body | `clearance_cm` >= 0 for item ends outside the gripping arm (tune `body_radius` to the character's bulk: 8 cm slim, 12-15 cm armoured) |
| Contact that should happen | within 5 cm inside its time window |
| Contact that must not happen | never below its `distance` |
| Partner | no `partner_clipping` except at intended contacts |
| Feet | no sample more than 2 cm below ground; planted feet move < 2 cm between neighbouring samples |

## 2. Axes, handedness and units

Unreal is **left-handed, Z up, X forward, Y right, centimetres**. Consequences:

- **Skeletal mesh assets usually face +Y** in their own space (the UE mannequin does), and a
  Character Blueprint rotates the mesh component -90 degrees yaw so it faces the actor's +X. Do
  not "fix" facing by rotating bones; fix the component rotation or the import.
- **Left and right are relative to the character**, not to the camera or the world. A front view
  mirrors the character's right hand onto the image's left. `ue_anim_inspect` works this out from
  the skeleton's own left/right bone pairs (`_l`/`_r`, `Left`/`Right`, `.L`/`.R`), so trust its
  `side` over your reading of an image, and check `frame.left_right_pairs_found` > 0. If it is 0
  the skeleton has no recognisable pairs: pass `track` bones and reason from `fwd_right_up` with
  the axes it printed.
- **DCC tools differ** (Blender is right-handed Z-up, Maya is commonly Y-up), and FBX export
  converts axes. When a whole character or item arrives rotated 90 or 180 degrees, or mirrored,
  the fix is at export/import, not in Unreal transforms. Unreal's FBX import has `Convert Scene`,
  `Force Front X Axis` and `Convert Scene Unit` options (the Interchange pipeline in newer
  versions has equivalents - check the import dialog of your version). Record the settings that
  work in the project's docs so every import matches.
- **Units:** a 180 cm human should be about 180 units tall. Check with the mesh bounds
  (`get_bounds()` in Python). A character 100x too small or large means the export unit or the
  import unit conversion is wrong.
- **Never use negative scale to mirror** an item or a limb for the other hand: it flips normals,
  breaks physics and some attach math. Author or duplicate a mirrored socket instead.

## 3. Rig and skeleton checks

Do these once per character, before debugging individual animations:

- **Bone pairs exist and are named consistently** (`hand_l`/`hand_r`). Inconsistent names break
  mirroring, retarget chains and the side detection.
- **Reference pose**: run `ue_anim_inspect` with no animation and `track` both hands and feet. Hands
  must be on their own sides, feet near height 0. A reference pose with arms crossed or a flipped
  pelvis means a bad export.
- **Animations share the skeleton** (or a compatible one). An animation on the wrong skeleton plays
  with scrambled limbs; retarget it instead.
- **Retargeting (IK Rig + IK Retargeter)**: chains must map left to left. A swapped chain gives a
  character that raises the wrong arm. Source and target retarget poses must match (A-pose vs
  T-pose mismatch gives arms that float away from or into the body). The retargeter UI and
  options were reworked in 5.6; follow the version you have.
- **Proportions**: animations made for a different body (longer arms, bigger torso) give grips
  that miss and hands that clip the chest. Fix with IK (section 5) or per-character offsets, not
  by editing every clip.

## 4. Attaching anything

The same rules hold for a sword, a hammer, a torch, a shield, a phone, a guitar, a backpack or a hat.

**Convention - set it once per project and write it down** (in the project's docs and in each
item's asset description):
- The item's **pivot is at the grip point** (where the palm closes), or the item has a `Grip`
  socket there.
- The item's **long axis** is a fixed axis (for example +X points from the grip toward the
  business end: blade tip, hammer head, torch flame).
- Two-handed items have a second socket for the off hand (for example `OffHand`).
- Worn items have their own sockets on the body (`back_socket`, `hip_l_socket`, `head_socket`).

**Where to attach:** to a **socket** on the character's skeleton or mesh, placed at the palm, not
to the raw hand bone. The socket's transform is the knob you tune; animations stay untouched.
Add or edit sockets in the Skeleton editor (with a preview mesh attached so you can see it), or
from Python (reference/inspect-recipes.md). One socket per hold style (`hand_r_sword`,
`hand_r_tool`), and a mirrored socket on the other hand for left-handed use.

**Attaching in code:**

```cpp
// Snap the item's root to the socket; keep the item's own scale.
Item->AttachToComponent(GetMesh(), FAttachmentTransformRules::SnapToTargetNotIncludingScale, TEXT("hand_r_tool"));
// Moving it (holster <-> hand) at a montage notify: same call with the other socket name.
```

`KeepRelativeTransform` or `KeepWorldTransform` rules keep whatever offset the item had, which is
the usual source of an item that sits a little off after being picked up. Use `SnapToTarget...`
and put the offset in the socket.

**Two-handed holds:** the main hand holds the item through the socket; the off hand is driven by
IK to the item's off-hand socket every frame (Two Bone IK or FABRIK in the Animation Blueprint,
or Control Rig). Without IK the off hand only lines up in the one clip it was authored for. Check
both grips with `grips` in `ue_anim_inspect`.

**Measure grips against the palm, not the hand bone.** On most skeletons (the UE mannequin
included) the hand bone's origin is at the wrist. Give each hand a palm socket (`hand_r_palm`,
`hand_l_palm`) and use it as the `bone` of a grip check; the check accepts sockets.

**Checks for every attachment:** `side` right, `grips` within tolerance, item ends not inside the
body, the long axis pointing where the design says at the key poses (`end_a_at_start` /
`end_b_at_start` and the images).

## 5. Contacts and IK

Anything that must touch something else at a moment in time is a contact:

- **Hands on objects** (door handles, levers, ledges, a ladder, a steering wheel): the object
  provides a target socket; the Animation Blueprint drives the hand to it with Two Bone IK or
  Control Rig over the contact window, blended in and out by a curve or notify state.
- **Feet on the ground**: Leg IK / foot placement with traces keeps feet planted on slopes and
  steps. Foot sliding is a speed mismatch, not an IK problem - match movement speed to the clip or
  use stride/distance matching (`unreal-animation`).
- **Look and aim**: aim offsets and look-at, clamped to believable ranges.
- Express each contact as a `contacts` entry with `expect: "touch"` and a `window: [start, end]`
  so `ue_anim_inspect` checks it at every sample inside that window.

## 6. Two characters: hits, grabs, hugs, carries, handshakes

- **Spacing comes first.** Paired animations assume a fixed distance and facing between the two
  roots. Find it from the clips (or the design), then enforce it at runtime: Motion Warping to a
  warp target for the attacker, or snap/align both characters before a synced pair starts.
  Capsule radii set how close characters can get; if the animation needs them closer than the
  capsules allow, adjust spacing during the move (ignore pawn collision between the pair for its
  duration) rather than shrinking capsules globally.
- **Timing**: the contact frame of one clip must line up with the reaction frame of the other.
  Use `partner.time_offset` in `ue_anim_inspect` to line them up and to find the offset the game
  needs.
- **Check both directions**: `partner` reports this character's hands, feet, head and items against
  the partner's body. Run it again with the roles swapped to check the partner's limbs against
  this character.
- Intended touches (a hand on a shoulder, arms around a back) go in `contacts` with `expect:
  "touch"`; everything else should keep clearance >= 0.

## 7. Clipping

| Clips into | Usual fixes, cheapest first |
|---|---|
| Own body (item through torso or leg) | socket rotation/offset; a different hold socket for this move; hand IK target moved outward; widen the pose with an additive layer; last, a new clip |
| Own body (arms through chest with bulky armour) | per-character arm spread additive or Control Rig offsets; a slimmer collision assumption is not a fix |
| Another character | spacing and alignment (section 6); timing offset; Motion Warping; IK to a surface point on the other character |
| The environment | movement collision (the capsule) does not cover arms and items - trace-based IK, shorter item, or gameplay rules that stop the move near walls |

Cloth and hair clipping is a physics asset / cloth setup problem, not an animation one: check the
physics asset bodies around the area first.

## 8. Symptoms, causes, fixes

See reference/symptoms.md for the full table. The ones that come up most:

- **Item in the wrong hand** → attached to the wrong socket or bone (or a socket named `_r` that
  sits on `hand_l`); fix the socket's parent bone or the attach call. Confirm with
  `attachments[name].attach` and `side`.
- **Item on the right hand but pointing backwards or sideways** → the item's long axis does not
  follow the project convention, or the socket is rotated; fix the socket rotation (or re-export
  the item), measure `end_a_at_start` again.
- **Grip floats 5-15 cm off the palm** → item pivot not at the grip, or socket not at the palm;
  move the socket, or add a `Grip` socket and offset the attachment by its inverse.
- **Everything mirrored** (character raises left arm when it should raise right) → retarget chain
  swap, a mirrored export, or a Mirror node / mirror data table applied unintentionally.
- **Right in one clip, wrong in the next** → per-clip difference in how the hand was animated; use
  hand IK to the item so the hold is consistent, or per-clip socket overrides.

## 9. Flow and feel

A clip can pass every spatial check and still feel wrong. Check these with the preview images
and the clip timings:

- **Anticipation, action, recovery**: an attack or throw needs a readable wind-up, a fast action
  phase and a recovery. Too short a wind-up reads as no telegraph; too long feels sluggish. Put
  gameplay events (hit windows, releases) on notifies inside the action phase, not at its start.
- **Silhouette at key poses** (wind-up, contact, follow-through) should read clearly from the
  gameplay camera. Ask for a `three_quarter` view that matches it.
- **Transitions**: blend times of roughly 0.1-0.25 s between locomotion states; interrupts
  (hit reactions, dodges) need shorter blends or inertialization. Visible pops mean a missing
  transition or a mismatched start pose.
- **Weight**: heavier items (hammers) need slower wind-ups, more follow-through and root/hips that
  commit; light items can be snappy. Speeding a heavy clip up with play rate makes it look floaty.
- **Root motion vs in-place** must match how the move is driven (`unreal-animation`); mismatches
  cause sliding or snapping back.

Describe feel problems to the human with the timings you measured (frames at 30 fps), not
adjectives.

## 10. What needs a human

You cannot author new animation curves by hand in a sensible way, model a mesh, or paint skin
weights. When the fix needs any of these, say exactly what to make: which clip, which frames,
what the pose should be at each key time (in the `fwd_right_up` terms the tools report), and the
measured problem it solves. Offer marketplace or sample content (for example the Game Animation
Sample for locomotion) when it fits.

## Verify your work

- [ ] Measured the state before changing it.
- [ ] `ue_anim_inspect` passes with the real attachments, grips, partner and contacts of the move.
- [ ] `ue_anim_preview` images checked at the key times from front and side (and the gameplay
      camera angle), and they match the design.
- [ ] Sides stated in the report are the character's sides.
- [ ] Every change is in data (sockets, attach calls, IK targets, notifies) unless a new clip was
      unavoidable, and the report says which.
- [ ] Anything unverified is reported as unverified.

Recipes for common setups (one-handed item, two-handed item, shield, worn item, two-character
interactions) and Python for sockets: reference/inspect-recipes.md.
