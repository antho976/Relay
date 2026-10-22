// What webpack hands back for an imported file: its URL in the bundle.
declare module '*.woff2' {
  const url: string;
  export default url;
}
declare module '*.png' {
  const url: string;
  export default url;
}
declare module '*.jpg' {
  const url: string;
  export default url;
}
declare module '*.svg' {
  const url: string;
  export default url;
}

// Remotion hands a bundle the environment it renders with (RELAY_PROBE, and what a video sets).
declare const process: { env: Record<string, string | undefined> };
