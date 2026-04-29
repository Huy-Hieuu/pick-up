# Phase 1 — Game Service

The social core of PickUp. A "game" is a pickup sports session tied to a booked court slot. Players join, the game has a real-time WebSocket lobby, and once enough players join, bills are automatically split.

---

## REST API Endpoints

### POST /games — Create game

Creates a pickup game for a booked court slot. The creator is automatically added as the first player.

**Request:**
```json
{
  "court_slot_id": "uuid",
  "sport_type": "pickleball",
  "max_players": 6,
  "description": "Friendly game, all levels welcome"
}
```

**Response:** `201 Created`
```json
{
  "id": "uuid",
  "court_slot_id": "uuid",
  "creator_id": "user-uuid",
  "sport_type": "pickleball",
  "max_players": 6,
  "description": "Friendly game, all levels welcome",
  "status": "open",
  "players": [
    {
      "user_id": "user-uuid",
      "display_name": "Hieu",
      "avatar_url": "https://...",
      "joined_at": "2026-05-01T10:00:00Z"
    }
  ],
  "created_at": "2026-05-01T10:00:00Z"
}
```

**Errors:**
- `404` — court slot not found
- `409` — court slot not booked, or game already exists for this slot
- `422` — invalid max_players (must be 2–30)

**Flow:**
1. Verify the court slot exists and is `booked`
2. Verify no other game already references this slot
3. Create `games` row with status `open`
4. Insert creator into `game_players`
5. Return game with player list

---

### GET /games — List open games

**Query parameters:**

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `sport_type` | string | No | Filter by sport type |
| `date` | string | No | Filter by date (YYYY-MM-DD) |
| `lat` | float | No | User latitude |
| `lng` | float | No | User longitude |
| `radius_km` | float | No | Search radius (default: 10) |
| `page` | int | No | Page number (default: 1) |

**Response:** `200 OK`
```json
{
  "games": [
    {
      "id": "uuid",
      "sport_type": "pickleball",
      "court": {
        "id": "uuid",
        "name": "Pickleball Thao Dien",
        "address": "123 Thao Dien, D2",
        "distance_km": 2.4
      },
      "slot": {
        "start_time": "2026-05-01T18:00:00+07:00",
        "end_time": "2026-05-01T19:00:00+07:00"
      },
      "max_players": 6,
      "current_players": 3,
      "status": "open",
      "creator": {
        "display_name": "Hieu",
        "avatar_url": "https://..."
      }
    }
  ],
  "total": 8,
  "page": 1
}
```

Only returns games with status `open` or `full`.

---

### GET /games/:id — Game detail

Returns full game info including all players and payment summary.

**Response:** `200 OK`
```json
{
  "id": "uuid",
  "court_slot_id": "uuid",
  "creator_id": "user-uuid",
  "sport_type": "pickleball",
  "max_players": 6,
  "description": "Friendly game, all levels welcome",
  "status": "open",
  "court": {
    "id": "uuid",
    "name": "Pickleball Thao Dien",
    "address": "123 Thao Dien, D2",
    "price_per_slot": 300000
  },
  "slot": {
    "start_time": "2026-05-01T18:00:00+07:00",
    "end_time": "2026-05-01T19:00:00+07:00"
  },
  "players": [
    {
      "user_id": "uuid",
      "display_name": "Hieu",
      "avatar_url": "https://...",
      "joined_at": "2026-05-01T10:00:00Z",
      "payment_status": "paid"
    }
  ],
  "split": {
    "total_amount": 300000,
    "per_player_amount": 50000,
    "paid_count": 2,
    "total_count": 6
  },
  "created_at": "2026-05-01T10:00:00Z"
}
```

---

### POST /games/:id/join — Join game

**Response:** `200 OK` with updated game detail.

**Errors:**
- `404` — game not found
- `409` — game is full, already joined, or game not in joinable state
- `409` — user has another game at the same time slot

**Flow:**
1. Check game exists and status is `open`
2. Check user is not already a player
3. Check user doesn't have another game at the overlapping time
4. Insert into `game_players`
5. If `player_count == max_players`, set status to `full`
6. Broadcast `player_joined` to WebSocket lobby
7. Recalculate bill split (notify via WebSocket)
8. Return updated game detail

---

### POST /games/:id/leave — Leave game

**Errors:**
- `409` — creator cannot leave (must cancel instead)
- `404` — not a member of this game

**Flow:**
1. Check user is a member and not the creator
2. Remove from `game_players`
3. If status was `full`, set back to `open`
4. Broadcast `player_left` to WebSocket lobby
5. Recalculate bill split
6. Return updated game detail

---

### POST /games/:id/cancel — Cancel game

Creator-only. Sets game status to `cancelled`.

**Flow:**
1. Verify caller is the creator
2. Set status to `cancelled`
3. Broadcast `game_cancelled` to WebSocket lobby
4. Mark all `pending` payments for refund consideration

---

### PATCH /games/:id/status — Update game status

Creator-only. Transitions game between states.

**Request:**
```json
{
  "status": "in_progress"
}
```

**Valid transitions:**
- `open` or `full` → `in_progress`
- `in_progress` → `completed`

---

### GET /games/:id/share — Share deeplink

Returns a shareable URL for the game.

**Response:** `200 OK`
```json
{
  "url": "https://pickup.app/game/uuid",
  "message": "Join my pickleball game at Pickleball Thao Dien on May 1 at 6 PM! 3/6 spots left."
}
```

---

## Game Status State Machine

```
                    ┌─────────────────────────┐
                    │                         │
                    ▼                         │
              ┌──────────┐   max_players   ┌──────┐
  created ──► │   open   │ ──────────────► │ full │
              └────┬─────┘ ◄────────────── └──┬───┘
                   │        player_leaves     │
                   │                          │
                   ├──────────────────────────┤
                   │   creator starts game    │
                   ▼                          ▼
            ┌──────────────┐
            │ in_progress  │
            └──────┬───────┘
                   │  creator ends game
                   ▼
            ┌──────────────┐
            │  completed   │
            └──────────────┘

  Any state ──► cancelled  (creator action)
```

---

## WebSocket Protocol

### Connection

```
ws://host/ws/games/:id?token=<jwt>
```

- JWT is passed as a query parameter (WebSocket handshake doesn't support Authorization header)
- JWT is validated during the upgrade handshake
- Invalid/expired token → connection rejected with 401

### Architecture

```
                  ┌──────────────────────────────┐
                  │     Game Lobby Manager        │
                  │                               │
                  │  HashMap<GameId, broadcast::   │
                  │          Sender<WsMessage>>   │
                  │                               │
                  └──────────────────────────────┘
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
         ┌────────┐   ┌────────┐   ┌────────┐
         │Client A│   │Client B│   │Client C│
         └────────┘   └────────┘   └────────┘
```

Each game has a `tokio::sync::broadcast` channel, created lazily on first connection. When the last client disconnects, the channel is dropped.

### Server → Client Messages

All messages are JSON with a `type` field:

**player_joined**
```json
{
  "type": "player_joined",
  "user_id": "uuid",
  "display_name": "Minh",
  "avatar_url": "https://...",
  "player_count": 4,
  "max_players": 6
}
```

**player_left**
```json
{
  "type": "player_left",
  "user_id": "uuid",
  "player_count": 3,
  "max_players": 6
}
```

**game_status_changed**
```json
{
  "type": "game_status_changed",
  "status": "in_progress"
}
```

**payment_updated**
```json
{
  "type": "payment_updated",
  "user_id": "uuid",
  "payment_status": "paid",
  "paid_count": 3,
  "total_count": 6,
  "per_player_amount": 50000
}
```

**split_recalculated**
```json
{
  "type": "split_recalculated",
  "total_amount": 300000,
  "per_player_amount": 50000,
  "player_count": 6
}
```

**game_cancelled**
```json
{
  "type": "game_cancelled",
  "reason": "Creator cancelled the game"
}
```

### Client → Server

No client-to-server messages in P1. The WebSocket is broadcast-only (server pushes events). Chat is planned for P3.

### Connection Lifecycle

1. Client opens game detail screen → connects to WebSocket
2. Server adds client to the game's broadcast channel subscriber list
3. Server sends events as game state changes
4. Client closes screen → disconnects
5. Server pings every 30 seconds, drops connections that miss 2 pings (60s timeout)
6. Client auto-reconnects on unexpected disconnect (handled in mobile `useGame` hook)

---

## Business Rules

- `max_players` must be between 2 and 30
- A user cannot be in two games that overlap in time
- Game creator is always the first player and cannot leave (must cancel)
- Only games with status `open` or `full` appear in the public game list
- When a player joins or leaves, the bill split is recalculated and broadcast to all WebSocket clients
- A game can only be created for a slot that is `booked`
- One game per court slot

---

## Data Models (Rust)

```rust
// server/src/models/game.rs
pub struct Game {
    pub id: Uuid,
    pub court_slot_id: Uuid,
    pub creator_id: Uuid,
    pub sport_type: SportType,
    pub max_players: i16,
    pub description: Option<String>,
    pub status: GameStatus,
    pub created_at: DateTime<Utc>,
}

pub struct GamePlayer {
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub joined_at: DateTime<Utc>,
}

pub struct GameWithDetails {
    pub game: Game,
    pub court: Court,
    pub slot: CourtSlot,
    pub players: Vec<PlayerWithPayment>,
    pub split: SplitInfo,
}

pub struct PlayerWithPayment {
    pub user_id: Uuid,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub joined_at: DateTime<Utc>,
    pub payment_status: Option<PaymentStatus>,
}

pub struct CreateGameRequest {
    pub court_slot_id: Uuid,
    pub sport_type: SportType,
    pub max_players: i16,
    pub description: Option<String>,
}

// WebSocket message types
pub enum WsMessage {
    PlayerJoined { user_id: Uuid, display_name: String, player_count: i16, max_players: i16 },
    PlayerLeft { user_id: Uuid, player_count: i16 },
    GameStatusChanged { status: GameStatus },
    PaymentUpdated { user_id: Uuid, status: PaymentStatus, paid_count: i32, total_count: i32 },
    SplitRecalculated { total_amount: i32, per_player_amount: i32, player_count: i32 },
    GameCancelled { reason: String },
}
```

---

## Files to Implement

| File | Purpose |
|------|---------|
| `server/src/routes/games.rs` | HTTP handlers for /games/* endpoints |
| `server/src/services/game_service.rs` | Game creation, join/leave, status transitions |
| `server/src/db/games.rs` | Game + player SQL queries |
| `server/src/models/game.rs` | Game, GamePlayer, request/response structs |
| `server/src/ws/mod.rs` | WebSocket upgrade handler |
| `server/src/ws/game_lobby.rs` | Broadcast channel management, message types |

---

## Related Docs

- [Court Service](./p1-court-service.md) — games reference `court_slot_id`
- [Payment Service](./p1-payment-service.md) — join/leave triggers split recalculation
- [Database Schema](./p1-database-schema.md) — `games` and `game_players` tables
- [External Integrations](./p1-external-integrations.md) — Zalo deeplink for game sharing
- [Mobile App](./p1-mobile-app.md) — `useGame` hook manages WebSocket
