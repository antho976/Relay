# ElevenLabs source material

These seven MP3s were generated with the ElevenLabs `sound-generation` API
(44.1 kHz stereo MP3). They are checked in because the API is not
deterministic — the same prompt returns different audio each call, so these
files, not the prompts, are the reproducible source. `tools/make-sfx.sh`
trims, EQs, normalises and converts them into the delivered WAVs; it never
re-generates them.

To regenerate a variant by hand:

    KEY=$(sed -n 's/^ELEVENLABS_API_KEY[:=][[:space:]]*//p' .env | tr -d '\r\n')
    curl -s -X POST https://api.elevenlabs.io/v1/sound-generation \
      -H "xi-api-key: $KEY" -H "Content-Type: application/json" \
      -d '{"text":"<prompt>","duration_seconds":<n>,"prompt_influence":<n>}' -o out.mp3

| file | duration_s | prompt_influence | prompt |
|---|---|---|---|
| ding.mp3 | 1.2 | 0.6 | a single struck bell from a music box, warm and rounded, close microphone, dry, one clean strike with a pure sustained note, no room reverb, no hiss, no shimmer |
| swoosh.mp3 | 0.8 | 0.7 | a wooden domino card flicked quickly across a table and landing flat with a distinct tap, close mic, dry, brief motion then a clear landing, no wind, no air, no hiss |
| tick.mp3 | 0.5 | 0.7 | one press of a small mechanical keyboard switch, close mic, dry, crisp click with body, single event, no reverb, no hiss |
| screen.mp3 | 0.6 | 0.7 | a quiet short muted card placed down onto a smooth surface, close mic, dry, gentle soft contact, tiny and neutral, one event, no whoosh, no wind |
| fill.mp3 | 1.0 | 0.7 | an ascending musical run on a glass xylophone, notes going up the scale from low to high, landing on a final bright high note, dry close mic, no reverb |
| reveal.mp3 | 0.7 | 0.6 | a single soft mallet strike on a warm wooden marimba bar, close mic, dry, round gentle attack and short decay, one note only, no reverb |
| sweep.mp3 | 1.2 | 0.5 | a thick soft pillow pressed slowly and released, close mic, dry, deep muffled low movement, warm and dark, one gentle swell that fades |

## Prompts that FAILED, and why — read before writing new ones

A bare adjective ("a soft deep thud") reliably returns a formless sub with no
attack. Measured failures from this round:

- "a small brass chime bar struck once ... warm mellow"  -> 92% of energy in
  1-4 kHz but peaked at -28 dBFS, so normalising exposed the noise floor.
- "one soft muted felt mallet note on a warm low wooden block" -> 98.4% of
  energy below 200 Hz. Formless sub, no attack.
- "a soft muted felt pad tapped once on smooth glass" -> 100% below 200 Hz.
- "a deep resonant metallic wave travelling along a thick steel plate" ->
  97.8% below 200 Hz.
- "a small metal object thrown fast past the microphone" -> 99.2% ABOVE
  4 kHz. Pure hiss, the generic whoosh we were told to avoid.
- "a soft warm synthesizer tone rising smoothly in pitch" -> pitch actually
  FELL, 560 Hz to 275 Hz. Asking for "rising" is not enough; the word
  "ascending" plus "from a low pitch to a high pitch" is what worked.

What works: name the OBJECT and the STRIKE ("struck once with a hard mallet",
"landing flat with a distinct tap", "one press of a mechanical switch"),
state the mic position ("close mic"), and negate the failure mode explicitly
("no wind", "no air", "no hiss", "no reverb").

## The sweep rejection — the most useful data point here

The first sweep.mp3 ("a bright crystalline shimmer passing across a glass
panel...") was rejected by the director as sounding like "an electric cutter
cutting metal". The measurements had already said so: 24.1% of energy above
4 kHz, and the peak held for 320 ms. Bright inharmonic content that SUSTAINS
reads as a tool running rather than an event happening. Both halves matter —
a bright sound that decays fast is a tick; a dull sound that sustains is a
drone; bright plus sustained is a power tool.

The replacement round generated ten candidates against three hard limits
(<5% above 4 kHz, >=55% below 1 kHz, peak held <=120 ms) and found a
consistent trade-off worth knowing about:

- Material words that read as "soft and warm" ("woolen blanket", "pillow",
  "felt drum head") return sounds that are 94-99% below 200 Hz with a real
  swell shape, but almost no midrange — nearly inaudible on small speakers.
- Material words that read as "friction" ("cloth wiped across a tabletop",
  "leather glove on suede", "paper turning") return a usable warm midrange
  but an essentially FLAT envelope — the cloth-wipe take held its peak for
  480 ms, the same failure as the rejected version in a friendlier material.

Fade-shaping a flat take into an arc does not work: the underlying texture is
so even that the middle stays within 3 dB of peak regardless of the fades.
Layering a low swell under a midrange texture does not work either — the sub
so dominates the energy budget that the texture never registers in the split.

What worked was picking the take that already had the ARC and then
HIGHPASSING it, spending the normalisation headroom on the audible band
instead of on sub-bass no consumer device reproduces. Shape has to come from
the take; tone can be corrected afterwards.
