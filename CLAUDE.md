# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

**PickUp** — a mobile app combining sports court booking, pickup game creation with teammate matching, and automatic bill splitting. Targets males 18–35 playing outdoor sports on weekends in Vietnamese cities. Initial focus: pickleball and mini football in Ho Chi Minh City.

The bill-splitting flow is the #1 viral growth hook and should always receive the most polish.

## Architecture

Monorepo with three main packages:

- **`mobile/`** — Expo + React Native app (Expo Router for file-based routing, Zustand for state, custom hooks for data fetching)
- **`web-owner/`** — Court owner portal (Vite + React SPA) — Phase 2
- **`server/`** — Rust backend using Axum, SQLx (compile-time checked queries), PostgreSQL

Communication: REST + WebSocket (game lobby live updates). Auth: phone OTP → JWT.

### Backend structure (`server/src/`)

```
routes/     → Axum handlers (thin — delegate to services)
services/   → Business logic layer
models/     → SQLx row types + domain structs
db/         → Raw SQL queries via SQLx
extractors/ → JWT claims, validated JSON body
middleware/ → Auth verify, CORS, (later: rate limit, cache)
ws/         → WebSocket upgrade + game lobby handler
jobs/       → Background tasks via tokio (Phase 2+)
```

### Frontend structure (`mobile/`)

```
app/        → Expo Router pages (file-based routing)
  (auth)/   → Login/OTP flow
  (tabs)/   → Bottom tab navigator (explore, games, profile)
  court/    → Court detail + slot picker
  game/     → Create/join game, payments, ratings
src/
  api/      → API client layer (one file per domain)
  components/ → Shared UI components
  hooks/    → useAuth, useCourts, useGame, useLocation
  stores/   → Zustand stores (auth, game state)
  types/    → Shared TypeScript types
  utils/    → bill-split calc, formatting helpers
```

### Data layer

- PostgreSQL with SQLx migrations in `migrations/` (numbered SQL files)
- S3/MinIO for court photos and avatars
- Redis for caching (Phase 4)

### External integrations

Momo/ZaloPay (payments + webhooks), SMS provider (OTP), Google Maps (court locations), Expo Push (notifications — Phase 2), Zalo deeplink (game sharing)

## Build and development commands

```bash
# Backend
cd server && cargo build
cargo run                        # Start dev server
cargo test                       # Run all tests
cargo test test_name             # Run single test
cargo clippy                     # Lint
sqlx migrate run                 # Apply migrations
sqlx prepare                     # Regenerate offline query cache (.sqlx/)

# Mobile
cd mobile && npx expo start      # Start Expo dev server
npx expo start --ios             # iOS simulator
npx expo start --android         # Android emulator

# Infrastructure
docker compose up -d             # Start Postgres + services
```

## Phased roadmap

Reference `pickup_architecture_html_interactive.html` and `pickup_rust_rn_source_tree.html` in the project root for interactive architecture diagrams with phase filtering.

- **P1 (MVP, wk 5–12):** Auth, court booking, game creation/joining, bill splitting, WebSocket lobby
- **P2 (Iterate, wk 13–18):** Court owner portal, notifications, payment tracking, Fly.io deploy
- **P3 (Community, wk 19–24):** Player profiles, ratings, friends, game history/recaps
- **P4 (Scale):** Redis caching, full-text search (pg_trgm), rate limiting, monitoring

## Key conventions

- SQLx compile-time checked queries — run `sqlx prepare` after changing any SQL
- Axum handlers should be thin; business logic goes in `services/`
- API client in mobile: one file per domain in `src/api/`, wrapped by hooks in `src/hooks/`
- Slot booking uses `SELECT FOR UPDATE` for concurrency safety
- Payment webhooks (Momo/ZaloPay) live in `routes/webhooks.rs` — verify signatures before processing

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **pick-up** (2320 symbols, 5561 relationships, 176 execution flows).

> Index stale? Run `node .gitnexus/run.cjs analyze --index-only` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? Bootstrap with `npx`, `bunx`, or `pnpm dlx` — e.g. `bunx gitnexus@latest analyze` (npm 11 npx crash; #1939).

## Always Do

- **MUST run impact before editing.** Use `impact({target: "symbolName", direction: "upstream"})` or `node .gitnexus/run.cjs impact "symbolName" --direction upstream --repo .`; report callers, processes, and risk. Never substitute grep for graph analysis.
- **MUST analyze graph changes before committing.** Use `detect_changes({scope: "all"})` (MCP) or `node .gitnexus/run.cjs detect-changes --scope all --repo .` (CLI fallback). `partial: true` or `truncated: true` is not a clean check — a zero means unseen, not unaffected; re-run it. For regression review: `detect_changes({scope: "compare", base_ref: "main"})` or `node .gitnexus/run.cjs detect-changes --scope compare --base-ref "main" --repo .`.
- MUST warn on HIGH/CRITICAL `risk` pre-edit; never use `riskSharedAxes` to waive a HIGH/CRITICAL `risk` warning. Compare File/symbol: MCP File omits axes; Graph-RAG expands File.
- **MUST treat `risk: UNKNOWN` as unresolved, not as low.** An empty caller set is not evidence the symbol is unused — it can also mean the callers are not resolvable by the index (plain-object property access, dynamic dispatch, cross-language calls). `impact` pairs `UNKNOWN` with a `riskNote` saying so. Confirm with a text search before treating the symbol as safe to change or delete; do not proceed on the strength of a zero.
- **MUST use `query({search_query: "concept"})` for concepts/flows, `context({name: "symbolName"})` for a named symbol, or `impact` for blast radius, on read-only callers, dependencies, imports, or execution flow.** Graph first; text search only for empty/`UNKNOWN`/literals.
- For security review, `explain({target: "fileOrSymbol"})` lists taint findings (source→sink flows; needs `analyze --pdg`).

## Never Do

- NEVER edit a function, class, or method before MCP/CLI impact analysis.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis, and never read `UNKNOWN` as an all-clear — it means the walk could not answer, which is the one verdict that requires confirming by other means.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit before MCP/CLI graph change analysis.

## Resources

| Resource | Use for |
| --- | --- |
| `gitnexus://repo/pick-up/context` | Codebase overview, check index freshness |
| `gitnexus://repo/pick-up/clusters` | All functional areas |
| `gitnexus://repo/pick-up/processes` | All execution flows |
| `gitnexus://repo/pick-up/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
| --- | --- |
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
