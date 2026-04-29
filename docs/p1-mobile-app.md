# Phase 1 — Mobile App

Expo + React Native with file-based routing (Expo Router). The mobile app is the only client in P1 — the court owner portal is P2.

---

## Screen Map

```
app/
├── _layout.tsx              Root layout — auth gate
├── (auth)/
│   ├── login.tsx            Phone number input
│   └── verify.tsx           OTP verification
├── (tabs)/
│   ├── _layout.tsx          Bottom tab bar config
│   ├── explore.tsx          Court map + list
│   ├── games.tsx            My games
│   └── profile.tsx          User profile
├── court/
│   └── [id].tsx             Court detail + slot picker
└── game/
    ├── create.tsx           Create pickup game
    └── [id].tsx             Game detail + join + pay
```

---

## Navigation Flow

```
App Launch
    │
    ▼
_layout.tsx checks authStore.isAuthenticated
    │
    ├── false ──► (auth)/login.tsx
    │                 │
    │                 ▼ enters phone
    │             (auth)/verify.tsx
    │                 │
    │                 ▼ OTP verified → tokens stored
    │                 │
    └── true ───► (tabs)/explore.tsx
                      │
              ┌───────┼───────┐
              ▼       ▼       ▼
          explore   games   profile
              │       │
              ▼       ▼
        court/[id]  game/[id]
              │       ▲
              ▼       │
        game/create ──┘
```

### Deep Link Handling

Universal link `https://pickup.app/game/{id}` → Expo Router resolves to `game/[id].tsx`.

If user is not authenticated, the auth gate redirects to login first, then returns to the deep-linked screen after verification.

---

## State Management

### Auth Store

```typescript
// src/stores/auth.ts
interface AuthState {
  accessToken: string | null;
  refreshToken: string | null;
  user: User | null;
  isAuthenticated: boolean;

  login: (accessToken: string, refreshToken: string, user: User) => void;
  logout: () => void;
  updateTokens: (accessToken: string, refreshToken: string) => void;
  updateUser: (user: User) => void;
}
```

Tokens are persisted to `SecureStore` (expo-secure-store) and loaded on app launch.

### Game Store

```typescript
// src/stores/game.ts
interface GameState {
  activeGameId: string | null;
  setActiveGame: (gameId: string | null) => void;
}
```

Minimal store — most game state is managed by the `useGame` hook via the API and WebSocket.

---

## API Client

### Base Client

```typescript
// src/api/client.ts
const client = {
  async request<T>(method: string, path: string, body?: any): Promise<T> {
    const token = useAuthStore.getState().accessToken;
    const res = await fetch(`${API_BASE_URL}${path}`, {
      method,
      headers: {
        'Content-Type': 'application/json',
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
      body: body ? JSON.stringify(body) : undefined,
    });

    if (res.status === 401) {
      // Attempt token refresh
      const refreshed = await refreshAccessToken();
      if (refreshed) return client.request<T>(method, path, body);
      // Refresh failed — logout
      useAuthStore.getState().logout();
      throw new AuthError();
    }

    if (!res.ok) {
      const error = await res.json();
      throw new ApiError(res.status, error);
    }

    return res.json();
  },

  get: <T>(path: string) => client.request<T>('GET', path),
  post: <T>(path: string, body?: any) => client.request<T>('POST', path, body),
  patch: <T>(path: string, body?: any) => client.request<T>('PATCH', path, body),
};
```

### Domain API Modules

**src/api/auth.ts**
```typescript
requestOtp(phone: string): Promise<{ message: string; expires_in: number }>
verifyOtp(phone: string, code: string): Promise<AuthResponse>
refreshToken(token: string): Promise<{ access_token: string; refresh_token: string }>
getMe(): Promise<User>
updateMe(data: { display_name?: string }): Promise<User>
```

**src/api/courts.ts**
```typescript
getCourts(filters: CourtFilters): Promise<PaginatedResponse<CourtWithDistance>>
getCourtById(id: string): Promise<Court>
getSlots(courtId: string, date: string): Promise<{ slots: CourtSlot[] }>
bookSlot(courtId: string, slotId: string): Promise<CourtSlot>
```

**src/api/games.ts**
```typescript
getGames(filters: GameFilters): Promise<PaginatedResponse<GameSummary>>
getGameById(id: string): Promise<GameWithDetails>
createGame(data: CreateGameRequest): Promise<GameWithDetails>
joinGame(id: string): Promise<GameWithDetails>
leaveGame(id: string): Promise<GameWithDetails>
cancelGame(id: string): Promise<void>
updateGameStatus(id: string, status: string): Promise<GameWithDetails>
getShareLink(id: string): Promise<{ url: string; message: string }>
```

**src/api/payments.ts**
```typescript
getSplit(gameId: string): Promise<SplitInfo>
initiatePayment(gameId: string, provider: 'momo' | 'zalopay'): Promise<InitiatePaymentResponse>
getPayments(gameId: string): Promise<{ payments: Payment[] }>
```

---

## Custom Hooks

### useAuth

```typescript
// src/hooks/useAuth.ts
function useAuth() {
  const store = useAuthStore();
  const [loading, setLoading] = useState(false);

  const requestOtp = async (phone: string) => { ... };
  const verify = async (phone: string, code: string) => { ... };
  const logout = () => { ... };

  return { ...store, loading, requestOtp, verify, logout };
}
```

### useCourts

```typescript
// src/hooks/useCourts.ts
function useCourts(filters: CourtFilters) {
  const [courts, setCourts] = useState<CourtWithDistance[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  // Fetch on mount and when filters change
  // Return { courts, loading, error, refresh }
}
```

### useGame (with WebSocket)

The most complex hook — manages both REST data and a WebSocket connection.

```typescript
// src/hooks/useGame.ts
function useGame(gameId: string) {
  const [game, setGame] = useState<GameWithDetails | null>(null);
  const [isConnected, setIsConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);

  // 1. Fetch game detail via REST on mount
  // 2. Connect to WebSocket: ws://host/ws/games/{gameId}?token=<jwt>
  // 3. Handle incoming messages:
  //    - player_joined → update player list
  //    - player_left → update player list
  //    - game_status_changed → update status
  //    - payment_updated → update player payment status
  //    - split_recalculated → update split info
  //    - game_cancelled → show alert, navigate back
  // 4. Auto-reconnect on unexpected disconnect (exponential backoff)
  // 5. Disconnect on unmount

  return {
    game,
    isConnected,
    join: () => gamesApi.joinGame(gameId).then(setGame),
    leave: () => gamesApi.leaveGame(gameId).then(setGame),
    cancel: () => gamesApi.cancelGame(gameId),
    refresh: () => gamesApi.getGameById(gameId).then(setGame),
  };
}
```

### useLocation

```typescript
// src/hooks/useLocation.ts
function useLocation() {
  const [location, setLocation] = useState<{ lat: number; lng: number } | null>(null);
  const [permission, setPermission] = useState<'granted' | 'denied' | 'undetermined'>('undetermined');

  // Request expo-location permission on mount
  // Watch position with moderate accuracy
  // Return { location, permission, requestPermission }
}
```

---

## Key Screens

### (auth)/login.tsx

- Phone number input with `+84` prefix pre-filled
- "Send OTP" button
- Loading state while sending
- Error display for invalid phone or rate limit

### (auth)/verify.tsx

- 6-digit OTP input (auto-focus, auto-advance between digits)
- 60-second countdown timer before allowing resend
- Auto-submit when all 6 digits entered
- On success: store tokens, navigate to (tabs)

### (tabs)/explore.tsx

- Toggle between map view and list view
- Map: Google Maps with court markers, tapping opens court detail
- List: `CourtCard` components with name, sport, price, distance
- Filter bar: sport type chips (All, Pickleball, Mini Football)
- Requires location permission for distance sorting

### court/[id].tsx

- Court photo carousel (horizontal scroll)
- Court info: name, address, sport type, price
- `SlotPicker`: date strip (next 14 days) + time slot grid
- Available slots are tappable, booked slots are greyed out
- "Book & Create Game" button → books slot then navigates to game/create

### game/create.tsx

- Pre-filled with booked court and slot info
- Sport type selector
- Max players picker (2–30)
- Optional description field
- "Create Game" button → creates game → navigates to game/[id]

### game/[id].tsx (Game Detail — most important screen)

```
┌─────────────────────────────────┐
│  Pickleball Thao Dien           │
│  Thu May 1, 6:00 – 7:00 PM     │
│  Status: open (3/6 players)     │
├─────────────────────────────────┤
│                                 │
│  ┌─ Split ────────────────────┐ │
│  │ Total: 300,000 VND         │ │
│  │ Your share: 50,000 VND    │ │
│  │ ████████░░░░  2/6 paid     │ │
│  └────────────────────────────┘ │
│                                 │
│  ┌─ Players ──────────────────┐ │
│  │ 👤 Hieu (creator)    ✅ Paid│ │
│  │ 👤 Minh              ✅ Paid│ │
│  │ 👤 Lan               ⏳     │ │
│  │ 👤 3 spots open             │ │
│  └────────────────────────────┘ │
│                                 │
│  ┌────────────┐ ┌─────────────┐ │
│  │  Pay 50K   │ │   Share     │ │
│  └────────────┘ └─────────────┘ │
│                                 │
│  [ Join Game ]                  │
└─────────────────────────────────┘
```

- **SplitCard**: total, per-player amount, progress bar
- **PlayerList**: avatar + name + payment badge (green check / orange clock)
- **Pay button**: opens bottom sheet with Momo/ZaloPay choice → `Linking.openURL(payment_url)`
- **Share button**: opens system share sheet with game link + message
- **Join button**: shown for non-members, hidden once joined
- Real-time updates via `useGame` WebSocket

### Payment Bottom Sheet

```
┌─────────────────────────────────┐
│  Pay 50,000 VND                 │
│                                 │
│  ┌─────────────────────────────┐│
│  │  🟣 Momo                    ││
│  └─────────────────────────────┘│
│  ┌─────────────────────────────┐│
│  │  🔵 ZaloPay                 ││
│  └─────────────────────────────┘│
│                                 │
│  [ Cancel ]                     │
└─────────────────────────────────┘
```

Tapping a provider:
1. Calls `initiatePayment(gameId, provider)`
2. Opens `payment_url` via `Linking.openURL()`
3. User completes in provider app
4. On return to PickUp app (`AppState` listener), re-fetch payment status
5. WebSocket also pushes `payment_updated` in real-time

---

## Key Components

### SlotPicker

- Date strip: horizontal scroll of next 14 days (today highlighted)
- Time grid: vertical list of slots for selected date
- States: available (tappable, green), booked (greyed out), selected (highlighted)

### CourtCard

- Court thumbnail (first photo), name, sport type badge, price, distance
- Tappable → navigates to `court/[id]`

### PlayerList

- List of player avatars + names
- Payment status badge: green checkmark (paid), orange clock (pending), grey dash (not initiated)
- Creator has a "Creator" label
- Empty slots shown as "X spots open"

### SplitCard

- Total amount, per-player amount
- Progress bar: paid vs total players
- Paid/remaining summary

---

## TypeScript Types

```typescript
// src/types/user.ts
interface User {
  id: string;
  phone: string;
  display_name: string | null;
  avatar_url: string | null;
  created_at: string;
}

// src/types/court.ts
interface Court {
  id: string;
  name: string;
  sport_type: 'pickleball' | 'mini_football';
  lat: number;
  lng: number;
  address: string;
  price_per_slot: number;
  photo_urls: string[];
}

interface CourtSlot {
  id: string;
  start_time: string;
  end_time: string;
  status: 'available' | 'booked' | 'locked';
}

// src/types/game.ts
interface Game {
  id: string;
  court_slot_id: string;
  creator_id: string;
  sport_type: string;
  max_players: number;
  description: string | null;
  status: 'open' | 'full' | 'in_progress' | 'completed' | 'cancelled';
  created_at: string;
}

interface GameWithDetails extends Game {
  court: Court;
  slot: CourtSlot;
  players: PlayerWithPayment[];
  split: SplitInfo;
}

interface PlayerWithPayment {
  user_id: string;
  display_name: string | null;
  avatar_url: string | null;
  joined_at: string;
  payment_status: 'pending' | 'paid' | 'expired' | null;
  is_creator: boolean;
}

// src/types/payment.ts
interface SplitInfo {
  total_amount: number;
  player_count: number;
  per_player_amount: number;
  creator_amount: number;
  paid_count: number;
  paid_total: number;
  remaining_total: number;
}

interface Payment {
  id: string;
  user_id: string;
  amount: number;
  provider: 'momo' | 'zalopay';
  status: 'pending' | 'paid' | 'expired' | 'refunded';
  paid_at: string | null;
}

// src/types/api.ts
interface PaginatedResponse<T> {
  data: T[];
  total: number;
  page: number;
  per_page: number;
}

interface ApiError {
  error: string;
  message: string;
  details: any | null;
}
```

---

## Utilities

### bill-split.ts

Client-side split calculation for instant UI feedback (before server confirmation):

```typescript
// src/utils/bill-split.ts
function calculateSplit(totalAmount: number, playerCount: number, creatorOverride?: number) {
  if (creatorOverride !== undefined) {
    const remaining = totalAmount - creatorOverride;
    const perPlayer = Math.floor(remaining / (playerCount - 1));
    return { creatorAmount: creatorOverride, perPlayerAmount: perPlayer };
  }
  const perPlayer = Math.floor(totalAmount / playerCount);
  const remainder = totalAmount % playerCount;
  return { creatorAmount: perPlayer + remainder, perPlayerAmount: perPlayer };
}
```

### format.ts

```typescript
// src/utils/format.ts
function formatVND(amount: number): string      // "100,000 VND" or "100K"
function formatDate(iso: string): string         // "Thu May 1"
function formatTime(iso: string): string         // "6:00 PM"
function formatDistance(km: number): string       // "2.4 km" or "500 m"
```

---

## Files to Implement

| File | Purpose |
|------|---------|
| `mobile/app/_layout.tsx` | Root layout, auth gate |
| `mobile/app/(auth)/login.tsx` | Phone input screen |
| `mobile/app/(auth)/verify.tsx` | OTP verification screen |
| `mobile/app/(tabs)/_layout.tsx` | Tab bar configuration |
| `mobile/app/(tabs)/explore.tsx` | Court map + list |
| `mobile/app/(tabs)/games.tsx` | My games list |
| `mobile/app/(tabs)/profile.tsx` | User profile |
| `mobile/app/court/[id].tsx` | Court detail + slot picker |
| `mobile/app/game/create.tsx` | Create game form |
| `mobile/app/game/[id].tsx` | Game detail + join + pay |
| `mobile/src/api/client.ts` | Base API client |
| `mobile/src/api/auth.ts` | Auth endpoints |
| `mobile/src/api/courts.ts` | Court endpoints |
| `mobile/src/api/games.ts` | Game endpoints |
| `mobile/src/api/payments.ts` | Payment endpoints |
| `mobile/src/hooks/useAuth.ts` | Auth hook |
| `mobile/src/hooks/useCourts.ts` | Courts query hook |
| `mobile/src/hooks/useGame.ts` | Game + WebSocket hook |
| `mobile/src/hooks/useLocation.ts` | GPS hook |
| `mobile/src/stores/auth.ts` | Zustand auth store |
| `mobile/src/stores/game.ts` | Zustand game store |
| `mobile/src/components/SlotPicker.tsx` | Date + time slot selector |
| `mobile/src/components/CourtCard.tsx` | Court preview card |
| `mobile/src/components/PlayerList.tsx` | Player list with payment status |
| `mobile/src/components/SplitCard.tsx` | Bill split summary |
| `mobile/src/types/*.ts` | TypeScript type definitions |
| `mobile/src/utils/bill-split.ts` | Client-side split calculation |
| `mobile/src/utils/format.ts` | Formatting helpers |

---

## Related Docs

- [Auth Service](./p1-auth-service.md) — API contract for login/verify/refresh
- [Court Service](./p1-court-service.md) — API contract for courts and slots
- [Game Service](./p1-game-service.md) — API contract for games, WebSocket protocol
- [Payment Service](./p1-payment-service.md) — API contract for split and payments
- [External Integrations](./p1-external-integrations.md) — Google Maps setup, deep link config
