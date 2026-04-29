# Phase 1 — Court Service

Court discovery and time-slot booking. Courts are the physical locations where games happen. In P1, court data is seeded (no creation API) — the court owner portal for managing courts is P2.

---

## API Endpoints

### GET /courts — List courts

Browse courts filtered by sport type and/or location proximity. Public endpoint (no auth required).

**Query parameters:**

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `sport_type` | string | No | Filter: `pickleball`, `mini_football` |
| `lat` | float | No | User latitude (required with `lng`) |
| `lng` | float | No | User longitude (required with `lat`) |
| `radius_km` | float | No | Search radius in km (default: 10, max: 50) |
| `page` | int | No | Page number (default: 1) |
| `per_page` | int | No | Results per page (default: 20, max: 50) |

**Response:** `200 OK`
```json
{
  "courts": [
    {
      "id": "uuid",
      "name": "Pickleball Thao Dien",
      "sport_type": "pickleball",
      "lat": 10.8033,
      "lng": 106.7390,
      "address": "123 Thao Dien, District 2, HCMC",
      "price_per_slot": 300000,
      "photo_urls": ["https://..."],
      "distance_km": 2.4
    }
  ],
  "total": 15,
  "page": 1,
  "per_page": 20
}
```

**Distance calculation (Haversine SQL):**

```sql
SELECT *,
  (6371 * acos(
    cos(radians($1)) * cos(radians(lat)) *
    cos(radians(lng) - radians($2)) +
    sin(radians($1)) * sin(radians(lat))
  )) AS distance_km
FROM courts
WHERE sport_type = $3 OR $3 IS NULL
HAVING distance_km <= $4
ORDER BY distance_km ASC
LIMIT $5 OFFSET $6;
```

When `lat`/`lng` are not provided, results are ordered by `created_at DESC` and `distance_km` is `null`.

---

### GET /courts/:id — Court detail

**Response:** `200 OK`
```json
{
  "id": "uuid",
  "name": "Pickleball Thao Dien",
  "sport_type": "pickleball",
  "lat": 10.8033,
  "lng": 106.7390,
  "address": "123 Thao Dien, District 2, HCMC",
  "price_per_slot": 300000,
  "photo_urls": ["https://..."],
  "created_at": "2026-04-01T00:00:00Z"
}
```

**Errors:** `404` if court not found.

---

### GET /courts/:id/slots — Available slots

Returns slots for a specific date range.

**Query parameters:**

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `date` | string | Yes | Date in `YYYY-MM-DD` format |

**Response:** `200 OK`
```json
{
  "court_id": "uuid",
  "date": "2026-05-01",
  "slots": [
    {
      "id": "uuid",
      "start_time": "2026-05-01T06:00:00+07:00",
      "end_time": "2026-05-01T07:00:00+07:00",
      "status": "available"
    },
    {
      "id": "uuid",
      "start_time": "2026-05-01T07:00:00+07:00",
      "end_time": "2026-05-01T08:00:00+07:00",
      "status": "booked"
    }
  ]
}
```

Only returns slots for the requested date. `status` is one of: `available`, `booked`, `locked`.

---

### POST /courts/:id/slots/:slot_id/book — Book a slot

Requires JWT. Books a time slot with concurrency-safe locking.

**Response:** `200 OK`
```json
{
  "slot_id": "uuid",
  "court_id": "uuid",
  "start_time": "2026-05-01T06:00:00+07:00",
  "end_time": "2026-05-01T07:00:00+07:00",
  "status": "booked",
  "booked_by": "user-uuid"
}
```

**Errors:**
- `404` — court or slot not found
- `409` — slot already booked or locked

---

## Concurrency: SELECT FOR UPDATE

The booking flow uses PostgreSQL row-level locking to prevent double-booking.

### Transaction sequence

```sql
BEGIN;

-- 1. Lock the slot row (blocks other concurrent transactions)
SELECT id, status, court_id
FROM court_slots
WHERE id = $1 AND court_id = $2
FOR UPDATE;

-- 2. Check status in application code
-- If status != 'available' → ROLLBACK, return 409 Conflict

-- 3. Update the slot
UPDATE court_slots
SET status = 'booked', booked_by = $3
WHERE id = $1;

COMMIT;
```

### What happens with concurrent requests

```
User A                          User B
  │                               │
  │  BEGIN                        │  BEGIN
  │  SELECT ... FOR UPDATE        │  SELECT ... FOR UPDATE
  │  (acquires row lock)          │  (BLOCKED — waiting for lock)
  │  status = 'available' ✓       │
  │  UPDATE → 'booked'            │
  │  COMMIT (releases lock)       │
  │                               │  (lock acquired)
  │                               │  status = 'booked' ✗
  │                               │  ROLLBACK → 409 Conflict
```

User A succeeds. User B gets a 409 with a clear message: "This time slot is already booked."

### Timeout

The `SELECT FOR UPDATE` has an implicit wait. To prevent indefinite blocking, set a statement timeout:

```sql
SET LOCAL statement_timeout = '5s';
```

If the lock isn't acquired within 5 seconds, the transaction fails gracefully.

---

## Slot Generation

Courts have fixed-duration time slots. In P1, slots are pre-generated.

**Strategy:**
- Slots are generated for the next 14 days
- A SQL function or seed script creates hourly slots from 6:00 AM to 10:00 PM (16 slots/day)
- Slot duration matches the court's configuration (typically 60 minutes)
- New slots can be generated daily via a simple SQL script (P2 adds a background job)

```sql
-- Generate slots for a court for a given date
INSERT INTO court_slots (court_id, start_time, end_time)
SELECT
  $1,
  $2::date + (hour || ' hours')::interval,
  $2::date + ((hour + 1) || ' hours')::interval
FROM generate_series(6, 21) AS hour  -- 6 AM to 9 PM (last slot ends 10 PM)
ON CONFLICT DO NOTHING;
```

---

## Business Rules

- Courts are read-only in P1 — managed via seed data and migrations
- A user can book multiple slots, but not the same slot twice
- Only the booker can cancel a booking (sets slot back to `available`)
- Slots in the past cannot be booked
- The `locked` status is reserved for future use (payment hold in P2); in P1, booking is immediate

---

## Data Models (Rust)

```rust
// server/src/models/court.rs
pub struct Court {
    pub id: Uuid,
    pub name: String,
    pub sport_type: SportType,
    pub lat: f64,
    pub lng: f64,
    pub address: String,
    pub price_per_slot: i32,
    pub photo_urls: Vec<String>,
    pub owner_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

pub struct CourtSlot {
    pub id: Uuid,
    pub court_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub status: SlotStatus,
    pub booked_by: Option<Uuid>,
}

pub struct CourtListQuery {
    pub sport_type: Option<SportType>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub radius_km: Option<f64>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub struct CourtWithDistance {
    #[sqlx(flatten)]
    pub court: Court,
    pub distance_km: Option<f64>,
}
```

---

## Files to Implement

| File | Purpose |
|------|---------|
| `server/src/routes/courts.rs` | HTTP handlers for /courts/* endpoints |
| `server/src/services/court_service.rs` | Court listing, slot availability, booking logic |
| `server/src/db/courts.rs` | Court + slot SQL queries |
| `server/src/models/court.rs` | Court, CourtSlot, query/response structs |

---

## Related Docs

- [Game Service](./p1-game-service.md) — games reference a `court_slot_id`
- [Database Schema](./p1-database-schema.md) — `courts` and `court_slots` tables
- [API Gateway](./p1-api-gateway.md) — court listing is public, booking requires JWT
