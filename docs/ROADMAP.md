# Roadmap: AI traders

Plan for the AI paper-trading work started in #22 (provider-neutral MCP and
OpenAI-compatible model profiles). Each numbered item is meant to be one pull
request, merged in order within a phase. Follow `docs/COLLABORATION.md`.

Owner: **minoplhy** for the AI trader work unless noted; the maintainer
(**Ax-47**) reviews and merges. Claim an item by opening its draft PR.

## Phase 0: land #22

1. **Get #22 green and merged.**
   - Remove the duplicate `"Running…"` key in `packages/ui/src/i18n/thai.rs`
     (line 896 repeats line 360 with a different translation).
   - Rebuild and commit `packages/ui/assets/tailwind.css`.
   - Move the "Not implemented yet" list out of the PR body; this file tracks
     it now.
   - Let traders edit the theses of their own portfolio: add `save_thesis` and
     `add_thesis_note` to `ALLOWED_TOOLS`, add both to the tools whose
     `portfolio` argument is forced to the trader's portfolio, tell the trader
     prompt it may change its own theses without asking, and test that a call
     naming another portfolio still lands on the trader's own.

## Phase 1: safety first

Before traders run more often or on their own, they need hard limits.

2. **Per-run order limits**: maximum orders per run and maximum position size
   (share of portfolio value), enforced in the order tool, not in the prompt.
3. **Cash reserve and concentration limits**, configurable per portfolio.
4. **Emergency stop**: one switch that cancels active runs and blocks new ones
   across all AI portfolios.
5. **Approval mode**: the trader proposes orders; a person approves or
   rejects them before execution.

## Phase 1.5: a goal for each portfolio

Traders should know what they are trading for. Reuse the existing `goals`
table from `planning` (target, date, monthly contribution, `portfolio_id`)
rather than adding a second notion of goal.

5a. **Trader mandate**: add horizon, maximum acceptable drawdown and a
    benchmark ticker to `TraderConfig` (additive migration), editable next to
    the strategy in settings.
5b. **`get_my_goals` tool**: read-only, `portfolio` forced to the trader's
    own. Traders cannot create, change or delete goals.
5c. **Goals in the prompt**: at the start of each run, include the
    portfolio's goals, their `GoalProjection` (on track, required return) and
    the mandate, so the trader knows whether it is ahead or behind.

## Phase 2: model compatibility

6. **Mock OpenAI-compatible server for integration tests**, covering a full run
   with tool calls. Every later item in this phase adds cases to it.
7. **Per-profile settings**: timeout, max tokens, and custom headers beyond
   bearer authentication (headers stored with the secrets, not the profile).
8. **Retry and backoff** for 429 and 5xx responses, bounded by the run timeout.
9. **Capability detection** (tool calling, JSON mode) when a profile is
   tested, with a clear error when a model cannot call tools.
10. **Responses-style APIs** for providers without Chat Completions.
11. **Streaming** responses, shown live in the run view.

## Phase 3: memory across runs

12. **Decision summaries**: at the end of each run, store a short
    provider-neutral summary (what was done, why, open questions).
13. **Inject prior summaries** into the next run, within a size budget with
    oldest-first truncation.
14. **Memory UI**: view, edit and clear a trader's memory.

## Phase 4: automation

15. **Scheduled runs** per portfolio (only after Phase 1 is merged).
16. **Usage and estimated cost** per run and per profile.
17. **Notifications** when a run finishes or fails, using the existing
    notifications module.

## Phase 5: race mode

Needs Phases 1, 1.5, 2 (items 6–8) and 4 (item 15).

18. **Race entity and setup**: pick contestant portfolios, equal starting
    capital, duration, round frequency; strategy and model are locked while a
    race is active.
19. **Synchronised rounds**: freeze one market snapshot per round, run all
    contestants against it, and define what happens to slow or rate-limited
    contestants (the round closes at a deadline; late contestants skip it).
20. **Controls**: start, pause, resume, stop all.
21. **Leaderboard**: total return, drawdown, volatility, risk-adjusted return,
    progress toward each contestant's goal, return against its benchmark,
    fees, turnover, cash allocation and failed runs.
22. **Race history and export** of results and the tool-call audit.

## Open questions for the maintainer

- Should approval mode (item 5) be the default for new AI portfolios?
- What is an acceptable default cost ceiling per scheduled run?
- Is race mode a priority, or should memory and automation come first?
