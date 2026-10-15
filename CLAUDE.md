# CLAUDE.md - REA OnScreen Takeoff (clean-room takeoff + bid app)

> Persistent project memory. Read this file FULLY before doing anything else.
> It reflects the complete current state of the project, not the original plan.
> Companion doctrine: `THINKING.md` in this repo root (how to think, debug, and
> communicate). If you cannot access that file, the condensed rules in section 2
> below are binding on their own.

---

## 0. COLD-START BRIEFING (keep this section current, newest truth wins)

**Every agent updates this section before ending a session.** A new agent must be able
to read only section 0 and know exactly where things stand.

- **Project:** A clean-room rebuild of the On-Screen Takeoff (OST) feature set, later joined
  by a QuickBid-style bidding module, in one application for JC's estimating work.
- **Current phase:** Phase 0 (study) not started. Repo holds only CLAUDE.md, THINKING.md
  and .gitignore.
- **Where the work lives:** GitHub `deusmac/REA-OnscreenTakeoff`, branch
  `claude/rea-tools-software-analysis-rk9nak`. No PR yet. Working copy on JC's Windows PC
  is cloned from that branch.
- **How to run it:** Nothing to run yet.
- **How to verify changes:** No build or tests yet. Phase 0 output is documents in
  `docs/spec/`, verified by JC reading them against what OST actually does.
- **Last session did:** Set up the repo (cloud session, 2026-10-15): CLAUDE.md, THINKING.md,
  .gitignore. Agreed on the clean-room approach and the single combined app.
- **Next up (priority order):**
  1. On JC's PC: study OST behavior (drive the UI, screenshots, user manual, help files,
     exports) and write `docs/spec/ost-features.md`.
  2. Collect one real past job as reference data in `reference/` (plan PDFs, OST quantity
     export, QuickBid bid summary). See section 5 before committing anything there.
  3. Pick the stack and write the Phase 1 build plan.
- **Landmines:** Read section 5 clean-room rules before touching OST or QuickBid in any way.

## 1. Who you are working for

John Carlo Tiempo (JC), engr.jctiempo@gmail.com. Estimator and pre-construction manager at
Brightline Contracting, a commercial drywall, metal framing, and painting subcontractor in
Greenville, South Carolina. He works remotely from the Philippines (Asia/Manila). Strong
Python developer, pragmatic, ships tools for real construction workflows. His boss Jack
Morgan often reads the outputs on a phone. Full people-and-vendor context lives in
`C:\Users\User\Desktop\CLAUDE CONTEXT\CLAUDE.md`; read it when the task touches emails,
invoices, GCs, or company data.

**Style: act first, explain after.** JC grants initiative. Fix bugs and build improvements
without asking, within section 5 guardrails. No planning theater. Tight summaries.

## 2. The rules (condensed doctrine, binding for any model)

1. Understand before acting. Read this file fully. Read files before editing them.
2. Root cause before fix. Reproduce, isolate, prove the hypothesis, THEN fix. Never stack
   failed fixes; revert each failed attempt. Three strikes on a bug means write down what
   you learned here and regroup.
3. Plan before code for anything beyond a one-file change. Write the plan down.
4. Evidence before claims. Never say done or fixed without having run it and seen it work.
   Verify against real data when it exists.
5. Simplest thing that works. No speculative abstraction, dependencies, or config.
6. Git from minute one. Commit every working increment. Branch per feature.
7. Security ships with the feature. Every new route, query, or mutation gets its auth check
   and input validation the same day it is written. An endpoint without auth is a bug.
8. Instant UI feedback. Every user-facing write reacts immediately (optimistic update or
   loading state). Never a silent wait on a network round trip.
9. Cost discipline. Do not re-read specs already summarized here. Batch questions and
   parallel-run independent commands. No new MCP servers, plugins, or dependencies unless
   the task is impossible without them. Use cheap models or subagents for research and
   bulk work when the harness allows choosing.
10. Output conventions, everywhere, including code comments and product copy: no em dashes,
    no emojis, lists not tables in prose (tables inside app UIs are fine), phone numbers
    without a leading plus sign, plain professional tone in anything boss- or customer-facing.

## 3. Session protocol (this is how sessions get saved, do not skip)

**On start:**
1. Read this entire file.
2. If resume, memory, or checkpoint skills are available, use them. Otherwise section 0
   and section 9 ARE the memory.
3. Pull the branch first (`git pull`), since cloud and PC sessions both push to it.
4. State in one line what you are about to do, then do it.

**On end (mandatory, the session is not over until this is done):**
1. Update section 0 (cold-start briefing) to the new truth.
2. Add an entry to section 9 (changelog): date, what shipped, what broke, what is next.
   Section 9 is this project's changelog; there is no separate CHANGELOG.md.
3. Commit everything, including this file, and push the branch.
4. If checkpoint or memory-save skills are available, run them too.

## 4. Project specifics

- **Goal:** One app covering the full OST feature set first, then QuickBid-style bidding on
  the same data model, so quantities flow straight into bids with no second app.
- **Planned data model:** Project, then Plan sets and pages (with scale), then Conditions
  (linear, area, count, with height, waste and other multipliers), then Takeoff shapes,
  then Quantities, then Bid items, then Pricing (labor, material, markup).
- **Phases:**
  1. Phase 0: study OST and QuickBid behavior, write the spec.
  2. Phase 1: takeoff MVP (open PDF plans, set scale, linear/area/count conditions,
     totals, Excel export).
  3. Phase 2: rest of the OST feature set from the spec.
  4. Phase 3: bidding module (QuickBid equivalent) on the same data.
  5. Phase 4: hook into the company systems (for example the Athena bid estimate).
- **Stack:** Not decided. Leading proposal is a web app (React plus PDF.js plus canvas),
  with an optional desktop wrapper. Decide at the end of Phase 0.
- **Data sources:** Reference job files in `reference/` (once added). OST and QuickBid on
  JC's PC are study subjects only, never data sources for code.
- **Key files:** `CLAUDE.md`, `THINKING.md`, `docs/spec/` (to be created).
- **External services and keys:** None yet. Never hardcode keys, never commit .env.
- **Conventions in this codebase:** To be set with the stack.

## 5. Guardrails (do not cross without JC's explicit go-ahead)

**Clean-room rules (this project lives or dies on these):**
- Never decompile, disassemble, debug-attach to, or unpack OST or QuickBid binaries, DLLs,
  installers or their databases. Never read their proprietary project file internals.
- Study only through observable behavior and published material: using the app, screenshots,
  the user manuals and help files, training videos, and the exports the apps produce
  (Excel, CSV, PDF reports).
- Never copy their code, icons, images, UI artwork, help text, product names or branding
  into this repo. Specs describe behavior in our own words.
- Never modify the OST or QuickBid installation or JC's real project files. Work on copies.
- Before committing reference files, check they hold no client-confidential data JC does
  not want in GitHub. Keep the repo private.

**General:**
- Never write to live company data (production sheets, real email sending, real Drive files)
  during testing. Stage or dry-run instead.
- Never delete or overwrite originals of anything (emails, PDFs, backups).
- Never commit secrets. Never export credentials embedded in workflow or config files.

## 6. Verification playbook

No build tools yet. Minimum bar until they exist: run the program on a realistic input (a
real plan PDF from `reference/`) and paste the output into the session summary. Quantities
must match the OST export for the same job within rounding.

## 7. Shipping for colleagues

Anything a non-developer will use gets, before it is called finished:
- A one-click launcher (bat, vbs, or shortcut) that just works.
- A short plain-English guide (one page, no jargon) next to it.
- A test on the smallest screen it will realistically be used on.

## 8. Backlog

1. Phase 0 spec: OST features (`docs/spec/ost-features.md`).
2. Phase 0 spec: QuickBid features (`docs/spec/quickbid-features.md`), lower priority.
3. Reference job in `reference/`.
4. Stack decision and Phase 1 plan.

---

## 9. SESSION CHANGELOG (newest first; never delete old entries)

### 2026-10-15 - Session 1 (cloud)
- Shipped: repo setup with CLAUDE.md (from JC's universal template), THINKING.md, .gitignore.
- Learned: OST is Windows only, so the study phase must run on JC's PC. Clean-room rules
  agreed: behavior and docs only, no decompiling.
- Next: open the repo in Claude Code on the PC and start the OST behavior study.
