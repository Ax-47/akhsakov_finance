# Working together

Rules for everyone who changes this repository: the maintainer, contributors,
and the coding agents working for either. Agents follow these exactly; humans
may override them case by case.

## Branches and pull requests

- `main` is always releasable. Nobody pushes to it directly; everything lands
  through a pull request that the maintainer merges.
- One pull request does one thing. A feature, a fix, and a refactor are three
  PRs. If a PR description needs a "Not implemented yet" list longer than its
  "Implemented" list, split the work.
- Keep PRs small enough to review in one sitting: aim for under ~800 changed
  lines, excluding `Cargo.lock`, `thai.rs` and the generated `tailwind.css`.
- Open PRs as drafts. Mark one ready only when CI is green on its latest
  commit.
- Branch from the latest `main` and merge `main` back in when it moves. Never
  rebase or force-push a branch someone else also pushes to.
- Never push to another contributor's branch. Suggest the change in a review
  comment, or open a PR against their branch.

## Before you push

Run the same checks CI runs. A PR with red CI is not ready for review.

```sh
cargo clippy --workspace --all-targets --features api/server,ui/server
cargo test --workspace --features api/server,ui/server
cargo check -p web --features web --target wasm32-unknown-unknown
cargo clippy -p desktop --features desktop,server
(cd packages/ui && npm ci && npm run build:css)   # then commit assets/tailwind.css
```

- Any change to classes in `packages/ui/src` needs a rebuilt, committed
  `packages/ui/assets/tailwind.css`.
- Every new UI string is added once to `packages/ui/src/i18n/thai.rs`. Search
  for the English key first; the dictionary must not contain duplicates
  (`dictionary_has_no_duplicate_keys`). If the same English text needs a
  different Thai translation, use a different English key.
- Never skip, `#[ignore]`, or delete a test to get CI green. Fix the cause.

## Code

- Dioxus 0.7 only, as described in `AGENTS.md`. Respect `clippy.toml`: never
  hold a signal read or write across an `.await`.
- Follow the existing layout: `api` (server, split into controller / service /
  repository per feature), `dtos` (types shared by client and server), `ui`
  (components and pages), and thin `web` / `desktop` / `mobile` shells.
- Server code only compiles under the `server` feature; the web client must
  keep building for `wasm32-unknown-unknown`.
- New behaviour comes with tests next to it (`#[cfg(test)] mod tests`).
- Database changes are additive migrations. Never break an existing user's
  database on upgrade.

## UI

- Follow `docs/UI.md`. The maintainer owns the look and feel: any PR that
  adds or reshapes a screen needs their approval before it merges.
- A backend feature may ship with a minimal UI (a working control in the
  right place, all strings translated). Say so in the PR; the polished screen
  can follow in its own PR.
- PRs that change what the user sees attach screenshots: desktop and phone
  width, light and dark theme, English and Thai.

## Secrets and money

- API keys and other secrets live only in their own table (for example
  `ai_model_secrets`). They are never returned by an endpoint, logged,
  included in backups or exports, or sent anywhere except the provider they
  belong to.
- The app does paper trading only. Nothing may place real orders or move real
  money.
- AI traders act only on the portfolio they are assigned to, only through the
  allow-listed tools, and within the run limits. Any change that loosens a
  limit or adds a tool is its own PR and says so in its title.
- Every trader tool call has its `portfolio` argument forced to the trader's
  own portfolio on the server, whatever the model sent.
- AI traders may edit the theses of their own portfolio (logged in the journal
  as written by AI). They never edit the theses of other portfolios.
- Goals belong to the user. AI traders can read their portfolio's goals but
  never create, change or delete them.
- No vendor branding or vendor domains in product copy; keep the UI
  provider-neutral.

## Reviews and communication

- Reply to every review comment: fix it, or explain why not. Resolve a thread
  only after the fix is pushed.
- Write PR descriptions for a reviewer who has not seen your session: what
  changed, why, how it was tested, and what is intentionally left out.
- When two people (or agents) want to change the same area, agree in the PR or
  issue first. `docs/ROADMAP.md` says who owns what.
- If you are blocked, say what blocks you and what you need, in the PR.

## Commits

- Imperative, specific subject lines (`Models: retry 429s with backoff`), with
  a body when the reason is not obvious.
- Do not commit build output other than `tailwind.css` and the bundled
  `echarts.min.js`, and never commit `.env` files, databases, or keys.
