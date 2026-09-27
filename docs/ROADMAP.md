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

## Phase 1: safety first

Before traders run more often or on their own, they need hard limits.

2. **Per-run order limits**: maximum orders per run and maximum position size
   (share of portfolio value), enforced in the order tool, not in the prompt.
3. **Cash reserve and concentration limits**, configurable per portfolio.
4. **Emergency stop**: one switch that cancels active runs and blocks new ones
   across all AI portfolios.
5. **Approval mode**: the trader proposes orders; a person approves or
   rejects them before execution.

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

Needs Phases 1, 2 (items 6–8) and 4 (item 15).

18. **Race entity and setup**: pick contestant portfolios, equal starting
    capital, duration, round frequency; strategy and model are locked while a
    race is active.
19. **Synchronised rounds**: freeze one market snapshot per round, run all
    contestants against it, and define what happens to slow or rate-limited
    contestants (the round closes at a deadline; late contestants skip it).
20. **Controls**: start, pause, resume, stop all.
21. **Leaderboard**: total return, drawdown, volatility, risk-adjusted return,
    fees, turnover, cash allocation and failed runs.
22. **Race history and export** of results and the tool-call audit.

## Open questions for the maintainer

- Should approval mode (item 5) be the default for new AI portfolios?
- What is an acceptable default cost ceiling per scheduled run?
- Is race mode a priority, or should memory and automation come first?
