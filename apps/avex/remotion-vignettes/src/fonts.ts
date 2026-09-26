import {cancelRender, continueRender, delayRender, staticFile} from 'remotion';

/**
 * Registers the two faces the vignettes set type in (`public/fonts`, see its README) and holds the
 * render until both have loaded. Without this Chromium falls back to whatever `monospace` means on
 * the rendering machine, which is how the first cut of these ended up in a typewriter face nothing
 * in the app uses.
 */
const FACES = [
  {family: 'VignetteSans', file: 'fonts/NotoSans-Medium.ttf', weight: '500'},
  {family: 'VignetteMono', file: 'fonts/NotoSansMono-Regular.ttf', weight: '400'},
];

const handle = delayRender('Loading vignette fonts');

Promise.all(
  FACES.map(async ({family, file, weight}) => {
    const face = new FontFace(family, `url('${staticFile(file)}')`, {weight});
    await face.load();
    document.fonts.add(face);
  })
)
  .then(() => continueRender(handle))
  // A missing face must fail the render, not quietly fall back to the machine's monospace.
  .catch((err) => cancelRender(err));
