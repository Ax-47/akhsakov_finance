# Roadmap: AI traders

Plan for the AI paper-trading work started in #22 (provider-neutral MCP and
OpenAI-compatible model profiles). Each numbered item is meant to be one pull
request. Follow `docs/COLLABORATION.md`.

Owner: **minoplhy** for the AI trader work unless noted; the maintainer
(**Ax-47**) reviews and merges. Claim an item by opening its draft PR.

Item numbers stay fixed so issues and PRs can refer to them; there is no
item 2–5 since Phase 1 was dropped.

## Priority order

Work top to bottom. Done items are listed under their phase below.

| # | Item | Issue |
|---|---|---|
| 1 | Streaming (item 11) | #51 |
| 2 | Responses-style APIs (item 10) | #50 |

Why this order: the mock model server (item 6, done) came first because every
model change after it adds test cases there. Retries (item 8, done) came next,
because a single 429 used to fail a race round. Goals and the mandate change
what traders do, so they come before automation. The emergency stop and scheduled runs go together: portfolios use
paper money, but model API calls cost real money, and a schedule can run up a
bill unattended. Streaming and Responses-style APIs help fewer providers, so
they come last.

## Phase 0: land #22 (done)

1. **Get #22 green and merged.** Done. Traders the app runs itself got
   `save_thesis` and `add_thesis_note` later, in #56.

## Phase 1: dropped

Per-run order limits, cash reserve and concentration limits, and approval mode
(#42, #43, #45) were dropped: AI portfolios trade paper money only, so these
limits would not protect anything real. The emergency stop moved to Phase 4
as item 15a, because model API calls do cost real money.

## Phase 1.5: a goal for each portfolio (done)

Traders should know what they are trading for. Reuse the existing `goals`
table from `planning` (target, date, monthly contribution, `portfolio_id`)
rather than adding a second notion of goal.

5a. **Trader mandate**: add horizon, maximum acceptable drawdown and a
    benchmark ticker to `TraderConfig` (additive migration), editable next to
    the strategy in settings. Done (#57).
5b. **`get_my_goals` tool**: read-only, `portfolio` forced to the trader's
    own. Traders cannot create, change or delete goals. Done, for MCP
    clients and for traders the app runs (#56).
5c. **Goals in the prompt**: at the start of each run, include the
    portfolio's goals, their `GoalProjection` (on track, required return) and
    the mandate, so the trader knows whether it is ahead or behind. Done
    (#57).

## Phase 2: model compatibility

6. **Mock OpenAI-compatible server for integration tests**, covering a full run
   with tool calls. Every later item in this phase adds cases to it. Done
   (#46): `packages/api/src/models/mock_model.rs`, with whole runs tested in
   `run_tests.rs` next to it.
7. **Per-profile settings**: timeout, max tokens, and custom headers beyond
   bearer authentication (headers stored with the secrets, not the profile).
   Done (#47): the key can go in any header, and a manual run gets at least
   twice the profile's timeout.
8. **Retry and backoff** for 429 and 5xx responses, bounded by the run timeout.
   Done (#48): up to four attempts, honouring `Retry-After`, logged as
   `model_retry` run events, within the run or round deadline.
9. **Capability detection** (tool calling, JSON mode) when a profile is
   tested, with a clear error when a model cannot call tools. Done (#49):
   the result is kept with the profile, and odd tool-call shapes are
   normalised in one place.
10. **Responses-style APIs** for providers without Chat Completions. (#50)
11. **Streaming** responses, shown live in the run view. (#51)

## Phase 3: memory across runs (done)

12. **Decision summaries**: at the end of each run, store a short
    provider-neutral summary (what was done, why, open questions).
13. **Inject prior summaries** into the next run, within a size budget with
    oldest-first truncation.
14. **Memory UI**: view, edit and clear a trader's memory.

## Phase 4: automation

15. **Scheduled runs** per portfolio, together with or after item 15a. Done
    (#52): every N hours on chosen weekdays, optionally only while the US or
    Thai market is open; skipped while the emergency stop is on or the
    portfolio races.
15a. **Emergency stop**: one switch that cancels active runs and blocks new
     ones, scheduled ones included, across all AI portfolios. Done (#44):
     it also pauses running races and refuses every AI order, MCP included.
16. **Usage and estimated cost** per run and per profile. Done (#53): prices
    per million tokens are entered per profile, each run keeps its estimate,
    and each portfolio can cap cost per run and per month.
17. **Notifications** when a run finishes or fails, using the existing
    notifications module. Done (#54): failures only by default, or also
    trades and race results, or never; race rounds are summed up.

## Phase 5: race mode (done)

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

- What is an acceptable default cost ceiling per scheduled run? For now
  there is none: each portfolio's limits start blank.
