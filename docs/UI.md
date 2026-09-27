# UI guidelines

How the app should look, read and behave. Every pull request that changes
`packages/ui` follows this; the maintainer (**Ax-47**) has the final say on
anything visible.

## Where things go

- **Settings** is for setting things up once: connections, keys, appearance,
  data export. Nothing you use day to day lives there.
- Things you look at or act on often (an AI trader's runs, its decisions, its
  performance) belong on a page of their own or on the page of the thing they
  belong to (for example the portfolio page), not inside a settings card.
- A new page needs a route in `packages/web/src/main.rs`, an entry in the
  sidebar, and an entry in `PAGES` in `vim.rs`.
- Show the few things that matter first; put detail (full logs, raw data)
  behind a "Show details" toggle.

## Building blocks

Use what exists before writing new markup:

- `Page` / `PageHero` / `HeroStat` for a page's frame and header.
- `Card` for every section (title, optional subtitle and header actions).
- `Field` + `INPUT` for form rows; `Segmented` + `ToggleButton` for choices.
- `ActionButton` for a card's main action (`ButtonTone::Danger` for
  destructive ones), `GhostButton` for secondary actions.
- `Toasts` (`notify.rs`) for short confirmations ("Saved"), instead of a
  status line that stays on screen.
- `notify::poll_delay` / `sleep_ms` for timers and polling. Never write a
  `setTimeout` through `document::eval` again.
- Numbers through `format.rs` (`fmt_money`, `fmt_signed`, `fmt_shares`…);
  never print a raw `Decimal`, timestamp or UUID.

## Colours

Catppuccin tokens only (`ctp-*`), no hex values:

- text `ctp-text`, secondary `ctp-subtext0`, hints `ctp-overlay1`;
- main action and focus `ctp-mauve`;
- warning / "are you sure" `ctp-peach`, error `ctp-red`, success `ctp-green`;
- gains and losses through `signed_color`.

## Words

- Every string the user can see goes through `tr()` / `trf()`, including
  status messages, button labels and enum labels, with its Thai translation
  in `i18n/thai.rs` in the same PR.
- Write like the rest of the app: short, plain and friendly, addressed to the
  user. Say what it does for them, not how it is built.
  - Good: "Let an AI invest paper money for you and see how it does."
  - Not: "Reusable provider-neutral OpenAI-compatible API connections."
- Keep jargon (MCP, API, base URL, token) to the fields that need it, and
  explain it in the field's `hint`.
- Sentence case for titles and buttons ("Add funds", not "Add Funds").

## Behaviour

- Anything that deletes, resets, or can't be undone asks first, inline, the
  way the app already does: a `ctp-peach` sentence ending in "Continue?",
  then the action and "Cancel".
- Buttons that start work show that they are busy (`disabled` plus a label
  like "Saving…") and can't be pressed twice.
- Errors say what went wrong and what to do next, in the user's words, next
  to the thing that failed.
- Never show secrets, raw JSON, stack traces or internal IDs. Show a
  human summary; put technical detail behind "Show details".
- Every screen works at phone width (the app also ships for Android): no
  horizontal scrolling of the page, touch targets at least 40px high
  (`min-h-10`).
- Keep interactions keyboard-friendly; don't break the vim bindings.

## Before you ask for review

- Rebuild `packages/ui/assets/tailwind.css` (`npm run build:css`).
- Attach screenshots to the PR: desktop and phone width, light and dark
  theme, English and Thai, for every screen you changed.
- Say in the PR what you left rough on purpose.
