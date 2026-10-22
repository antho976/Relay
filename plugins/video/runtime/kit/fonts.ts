/// <reference path="./assets.d.ts" />
// Fonts. Four families come with the kit and are loaded for every video (relayRoot), so a video
// looks the same on any machine: Inter (100–900, italic too), Space Grotesk (300–700), Fraunces
// (a display serif, 100–900) and JetBrains Mono (100–800), all under the SIL Open Font License.
// A family the machine has (fc-list) also works; a font file the video brings goes in public/
// (video.asset.add) and is loaded with loadFonts. video.still flags any other family as
// `font_fallback`. Web font services are not reached at render time.
import { loadFont } from '@remotion/fonts';
import { staticFile } from 'remotion';
import fraunces from '@fontsource-variable/fraunces/files/fraunces-latin-wght-normal.woff2';
import frauncesExt from '@fontsource-variable/fraunces/files/fraunces-latin-ext-wght-normal.woff2';
import inter from '@fontsource-variable/inter/files/inter-latin-wght-normal.woff2';
import interExt from '@fontsource-variable/inter/files/inter-latin-ext-wght-normal.woff2';
import interItalic from '@fontsource-variable/inter/files/inter-latin-wght-italic.woff2';
import interItalicExt from '@fontsource-variable/inter/files/inter-latin-ext-wght-italic.woff2';
import mono from '@fontsource-variable/jetbrains-mono/files/jetbrains-mono-latin-wght-normal.woff2';
import monoExt from '@fontsource-variable/jetbrains-mono/files/jetbrains-mono-latin-ext-wght-normal.woff2';
import grotesk from '@fontsource-variable/space-grotesk/files/space-grotesk-latin-wght-normal.woff2';
import groteskExt from '@fontsource-variable/space-grotesk/files/space-grotesk-latin-ext-wght-normal.woff2';

const LATIN = 'U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD';
const LATIN_EXT = 'U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+0304,U+0308,U+0329,U+1D00-1DBF,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF';

/** The families the kit brings, as `fontFamily` names them. */
export const fonts = {
  inter: 'Inter',
  grotesk: '"Space Grotesk"',
  fraunces: 'Fraunces',
  mono: '"JetBrains Mono"',
} as const;

const BUNDLED: [family: string, weight: string, style: string, latin: string, ext: string][] = [
  ['Inter', '100 900', 'normal', inter, interExt],
  ['Inter', '100 900', 'italic', interItalic, interItalicExt],
  ['Space Grotesk', '300 700', 'normal', grotesk, groteskExt],
  ['Fraunces', '100 900', 'normal', fraunces, frauncesExt],
  ['JetBrains Mono', '100 800', 'normal', mono, monoExt],
];

let loaded = false;

/** Load the kit's families; relayRoot does, once. Each holds the first frame until it is ready. */
export const loadKitFonts = (): void => {
  if (loaded || typeof document === 'undefined') return;
  loaded = true;
  for (const [family, weight, style, latin, ext] of BUNDLED) {
    void loadFont({ family, url: latin, weight, style, unicodeRange: LATIN, format: 'woff2' });
    void loadFont({ family, url: ext, weight, style, unicodeRange: LATIN_EXT, format: 'woff2' });
  }
};

export type FontFile = {
  /** The family name to use in `fontFamily`. */
  family: string;
  /** Its file under public/, as `staticFile()` takes it: "assets/Brand-Bold.woff2". */
  file: string;
  /** "700", or a range for a variable font: "100 900". */
  weight?: string;
  style?: 'normal' | 'italic';
};

/** Load font files the video brings; call at module level (outside components), once each. */
export const loadFonts = (...files: FontFile[]): void => {
  for (const file of files) {
    void loadFont({ family: file.family, url: staticFile(file.file), weight: file.weight ?? '400', style: file.style ?? 'normal' });
  }
};
