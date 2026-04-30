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

This project is indexed by GitNexus as **pick-up** (1024 symbols, 1705 relationships, 35 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/pick-up/context` | Codebase overview, check index freshness |
| `gitnexus://repo/pick-up/clusters` | All functional areas |
| `gitnexus://repo/pick-up/processes` | All execution flows |
| `gitnexus://repo/pick-up/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
