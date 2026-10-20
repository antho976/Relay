# Pictures plugin for Relay

Turn this plugin on for a project whose designs need real photographs. It is off by default;
switch it on per project from the **Plugins** page or the plugin key on the project's row in the
sidebar. Agents and you then search free stock photos and place one on a design, with its
photographer and licence kept beside the file.

## What an agent gets

| Piece | Where it lands | When |
| --- | --- | --- |
| The `design-pictures` skill | `.claude/skills/design-pictures/`, `.agents/skills/design-pictures/` | As soon as the plugin is on |
| Working rules (`instructions.md`) | The agent's session brief | Every start, wake and resume |
| `design.picture.search`, `design.picture.thumb`, `design.picture.add` | The agent's tools | Its next start or resume; calls follow the switch at once |

In a project with Pictures off, agents are neither offered the three tools nor able to call them:
a call is refused `plugin.off`, with a hint to ask you. You are not held to the switch; the Design
view's image key offers **Turn on Pictures** where it is off.

## Switching it off

The tools leave the agents of that project and the search leaves its Design view. Photos already
placed stay, and so do their credits.

## What leaves this machine

Only the search words, sent to Openverse or Pexels, and the Pexels key, sent only to
`api.pexels.com`. Relay builds every request itself, from fixed addresses and the ids a search
answered: no agent or design can make it fetch a web address, and it never follows a redirect.
Searches are not kept in the audit log; placing a photo is, with its design and id.

## The two sources

| Source | Key | What this machine may ask anonymously |
| --- | --- | --- |
| [Openverse](https://openverse.org) (Creative Commons photos) | none | about 200 searches and 1,000 previews a day. Agents together may make 100 of the searches and fetch 300 previews in any 24 hours, so the rest of the day stays yours. |
| [Pexels](https://www.pexels.com) | yours, from pexels.com/api | 200 requests an hour, 20,000 a month, on your key |

When a source asks Relay to wait, Relay stops asking it until then and says so at once, never
retrying in a loop. Searches are kept in memory for an hour, and previews on disk under the
store's `picture-cache/` folder for a week after they were last looked at.

## The licences kept

Only photos anyone may use and change, commercially too, come through: **CC BY**, **CC BY-SA**,
**CC0**, **public domain** and the **Pexels License**. Anything else, or anything marked mature,
is left out of the results and refused if asked for by id. A CC BY-SA photo means work made from
it must be shared under the same licence; its label says so, and agents are told to tell you.

## Credits

A placed photo is stored in the store's `design-assets/` folder under a name made from its
content, and its credit (title, creator, licence, links, an attribution sentence and a caption
line) is written beside it as `<name>.credit.json`. `design.get` answers each picture's credit by
its `src`, whether the plugin is on or not, so an agent revising the design later still sees it.

## The Pexels key

You save it in the Design view's picker; Relay checks it with Pexels first and then keeps it in
the desktop keyring (Secret Service, through `secret-tool`), never in its store, its backups, its
logs or any answer. Agents cannot read it through Relay and are told never to ask for it.

- An agent's first Pexels search after the engine starts may raise the keyring's unlock prompt on
  your desktop, when the keyring is locked. Most desktops unlock it at login.
- Any program running as you, an agent's shell included, can still ask the keyring for it with
  `secret-tool lookup`. The key is a free search quota with no billing; the stronger model, a
  separate key holder, is described in `docs/KEY-PROXY.md`.
