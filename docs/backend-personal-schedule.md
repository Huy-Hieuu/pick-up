# Personal schedule API

Both endpoints require the current user's access token and never accept a user
ID selector. Data and totals use one read-only repeatable-read transaction.
`page` defaults to 1; `per_page` defaults to 20 and is bounded to 1–50.

`GET /auth/me/bookings?page=1&per_page=20` returns
`{ bookings, total, page, per_page }`. Each booking has `slot_id`, `court_id`,
`court_name`, `sport_type`, `start_time`, `end_time`, `status`, `booked_by`,
and nullable `game_id`. These are only slots still booked by the caller.
Future bookings come first in chronological order, followed by past bookings
most recently first. `game_id` is the noncancelled game occupying that slot;
null means a game can still be created. Cancelling a game releases its slot and
removes it from this booking list. A completed game retains its booking.

`GET /auth/me/games?page=1&per_page=20` returns the same
`{ games, total, page, per_page }` shape and game list items as `GET /games`.
It includes all statuses, sorted by slot start descending with stable ID ties.
The caller must be the creator, a current roster member, or a former member
with a paid contribution. Unpaid players who leave do not retain game history
through this endpoint. Creators retain cancelled games. Public discovery's
default open/full filter does not apply to personal history.

Clients should fetch these endpoints after sign-in and refresh on mutations,
so bookings and games remain recoverable when local app state is cleared.
Dates/times are ISO8601 UTC instants; discovery date filters and share text use
Vietnam calendar dates (`Asia/Ho_Chi_Minh`, UTC+07).
