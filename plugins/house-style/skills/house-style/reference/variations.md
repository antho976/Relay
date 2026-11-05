# Variations that really differ

When the owner asks for options, they see two or three variations beside the before (`../SKILL.md`); otherwise a redesign is one version. On a
Relay project they go on the design canvas (`house-style-native`). This file is for building them
as working code, when the piece is interactive or its motion is the question.

- **One piece per round.** If the brief spans a whole dashboard, take the highest-impact piece, say
  which and why, and offer the rest as later rounds. Restate the brief in one sentence: what it is,
  where it lives, what it must do.
- **Recon first:** stack, styling system, tokens, personality, and the context it renders in (the
  background and neighbours behind it, its sizes). Every variant uses the project's tokens so it
  could ship tomorrow.
- **Name each variant and its axis before building** (layout, density, interaction model, motion
  story): "Quiet", "Dense", "Editorial", never "Option A". Two variants that differ only in colour
  or copy are one variant, so replace one. Three by default and five at most. No two variants share
  a position on an axis.
- **Every variant fully works,** to the full craft bar: real interactions, real motion, real
  product-shaped copy and numbers. A sloppy variant loses on execution and teaches nothing.
- **Isolated from production:** an `/prototypes/<slug>` route with one file per variant, or one
  self-contained HTML file when there's no project. Nothing in production imports from it.
- **One variant at a time, full size, in real context** (a toast over a page, a card among siblings).
  Thumbnails distort spacing and scale. A small fixed picker (bottom-centre, plain dark chrome that
  is clearly not part of the design, number keys or arrows to flip, plus a replay key when motion is
  the question) switches **instantly**: flipping happens 100+ times a session, so it never animates.
- **Check before showing:** every variant renders and responds, and the console is clean. Then
  present `# | Variant | Axis | When it's right | Its cost` without picking a favourite, say where it
  runs and the keys, and stop. If asked which you'd choose, answer from the product's personality and
  frequency of use, not looks alone. If two variants converged while you built them, cut one and say
  so.
- **On a choice:** integrate the winner in the project's conventions and delete the prototype
  surface unless asked to keep it. "Riff on B" starts a new round diverging around B.
