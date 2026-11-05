# My list

My own rules for the house style. I edit this file by hand. It wins over everything else in the
house-style plugin, and only something I say in the conversation beats it. Agents read it before
any UI work and never edit it unless I ask. When I correct a design, the agent offers the new line
here for me to add; it does not add it itself.

Format: one rule per line. Add, delete or reword freely.

## Keep

- Keep the subject's own pictures: server banners, server and app icons, avatars, stream
  thumbnails. They are content, not decoration.
- A server's banner shows in two places: faded behind the page header, and as the server card at
  the top of the sidebar.
- The server icon is a rounded square with a ring in the accent colour, next to the page title.
- Clicking a picture I can change (the banner) changes it. No separate "Banner" button.

## Every page needs a reason to look at it

- Neutral chrome alone looks flat and plain. Each page gets at least one rich moment: a big
  picture, a chart in colour, or faces. "It doesn't make me want to see it" is a failure.
- A page about something with its own colour uses that colour for its data and its live state
  (Twitch purple on stream pages). The server's accent stays for actions and what needs me.
- One thing leads each page: a hero picture, a big number or a chart. Not a row of equal panels.
- Only main pages get that lead (overview, a feature's first tab). Sub-pages and tabs get the
  same panels and colour but no big header or lead number.
- A lead is compact: its height fits its content, and it never leaves an empty half beside it.
  "Fine" isn't the goal; a lead that is only fine gets redone.
- A subject's own colour shows where it belongs: Twitch purple on stream pages, gold for
  starboard stars. Never a grey box where the subject has a real mark.

## Charts

- A trend over time is a line, even over 7 days: the data-colour line with a soft fill, the period
  before as a dashed grey line, and a dot on today.
- The main chart may draw in once when the page opens.
- Charts draw in once, when the page opens. Changing a filter or a tab never redraws them.
- Small rankings (top givers, most given) are rows of one size with thin bars, not podiums or
  full-width bars.

## Layout and boxes

- Unboxed is fine on the overview. On settings pages (stream alerts and its tabs) every section
  sits in a bordered panel; without them it feels "all over the place".
- Placement matters more than borders: panels in a row end at the same height, no half-empty
  panels, a figure row never breaks a number across lines.
- Settings go beside the preview they change, with Save inside the settings panel. Side blocks
  sit under the settings, so the settings column and the preview column end together.
- Settings have hierarchy: each section opens with its heading and its on/off switch, related
  fields sit together (channel beside message), options group under what they belong to, and a
  label is never hundreds of pixels from its control.
- Discord previews are small (about 75%), one at a time with a switch between them, never two
  full-size previews stacked.
- Pictures in a gallery are small tiles (about 200 px) in a strip. A gallery never fills a 1080p
  screen; check it on one.
- Numbers that belong to a gallery or list ride in its panel's head, not in a panel of their own.
- People lists (who clips, who claimed, top inviters) go in the right column.
- Row action buttons in one list (Review, Open, Check) are all the same width.
- Rows in one list share one size: the same face size, title size and line size. A list that is
  bigger than its neighbours looks wrong.
- No icons on rows that don't need them (Running: the kind is a word in the line instead).

## Shell

- Moving between tabs or sub-pages inside one feature (Giveaways Running to Ended, a filter, All
  time / This week) keeps the header, tabs and sidebar still and only cross-fades what changed.
  The page never moves up and down. Switches always have that short cross-fade, never a cut.
- The notification bell sits beside Settings at the bottom of the sidebar, next to my account.
- The bell and the settings gear look the same size.
- The bell's dropdown is compact: a head with the new count and See all, notices of two lines at
  most with the feature and time under them, a dot for new ones, no chevrons.

## Process

- When I correct a design, keep my placement and fix only what I name; ask nothing I already said.
- Members can't open the dashboard, so anything they submit gets a command where they see its
  status and can remove it themselves, and the submit message tells them about that command.
- When I ask for a redesign, make one version unless I ask for options.
- Show me the real thing in a browser on localhost, and check every width before I look.
