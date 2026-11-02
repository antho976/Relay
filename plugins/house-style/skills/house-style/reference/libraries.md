# Libraries for React web work

Curated, opinionated picks from Emil Kowalski's list, for React and web projects only. GTK and Compose
use their platform's own controls (`house-style-native`).

1. **Name the task, not the library asked about.** "I need a dropdown" is a primitives task.
2. **Check `package.json` first.** Use a listed library that's already installed. If a competitor is
   installed (react-window rather than Virtuoso), mention the pick but don't churn the dependency
   unasked.
3. **Recommend one,** say what it's for in a sentence, and wire it up if that was asked. If the task
   isn't on the list, say you've left the list, then recommend from your own knowledge.

| Task | Library |
| --- | --- |
| Unstyled accessible primitives: dialogs, popovers, menus, selects | base-ui |
| Command menu (⌘K) | cmdk |
| Toasts | Sonner (load `ask-sonner` for its API and pitfalls) |
| One-time code input | input-otp |
| Tweakable control panel | Leva (dialkit as an alternative) |
| Springs, layout animation, exit animation, gestures | motion (Framer Motion); plain CSS for a fade or hover |
| Animated numbers | NumberFlow |
| Animated text | torph |
| 3D globe | Cobe |
| OG images from HTML/CSS | Satori |
| Syntax highlighting | shiki |
| Live, time-scrolling charts | Liveline |
| Any other chart | recharts |
| Drag and drop | dnd kit |
| Long lists and big tables | Virtuoso |
| Shared state | zustand |
| Conditional class names | clsx |
| Typed variants for Tailwind (size, intent, state) | cva |
| Theme switching with no flash on load | next-themes |

**Mismatches to catch:** toasts hand-built or made from a modal (Sonner); a `<div>` dropdown or dialog
with manual focus handling (base-ui); a number animated by re-rendering text (NumberFlow); 1,000+ rows
rendered directly (Virtuoso); a web of props for shared state (zustand); className ternaries three
deep (clsx, or cva when it's variant-shaped).
