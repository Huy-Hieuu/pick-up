# PickUp UI and feature map

The client is implemented in `mobile/`. The current release priority is Android.
The visual direction combines forest green, lime, a warm light background,
Inter typography and restrained rounded cards. The supplied hero asset illustrates
community sport; court photos, availability, prices and player counts come from
API data. Missing court photos use a sport illustration rather than a fabricated
venue photo or rating.

## Shared interface

`src/components/ui.tsx` defines colors, typography, buttons, fields, feedback,
loading/empty/error states and the scrolling page container. `AppShell.tsx`
provides bottom navigation on phones and header navigation on larger screens.
Pages use window dimensions, wrap filters and adjust grids at their breakpoints.
Text and button labels may wrap; user text remains scalable. Only the decorative
wordmark keeps its fixed font scale. Result grids center incomplete rows, state
and auth boxes use a centered maximum-width frame, and desktop profile/form
panels are centered within the content column.

The native shell accounts for safe-area insets, hides bottom navigation when the
keyboard opens and requests Android resize behavior. The brand mark is kept as
SVG with reproducibly rendered PNG launcher, adaptive and monochrome assets;
`mobile/scripts/render-brand.py` renders the PNGs with Pillow. The configured
splash screen shares the brand background and waits for font initialization.
Direct font/icon imports include four Inter weights and Feather.

Controls carry role, label and selected/disabled/busy state. Feedback uses a
polite live region. The Android touch-target baseline is 48 dp, following
[Android's accessibility guidance](https://developer.android.com/guide/topics/ui/accessibility/apps.html).
These properties follow [React Native accessibility APIs](https://reactnative.dev/docs/accessibility);
TalkBack behavior still requires device verification.

`src/components/motion.tsx` provides lightweight fade/slide entrances, route
transitions, active-navigation pills and press-scale feedback without an extra
animation dependency. Card lists use short staggered entrances. Every motion
primitive respects the operating system reduced-motion setting and keeps content
visible when animation is disabled.

## User journeys and verification

| Journey | Screen / context | API / evidence |
| --- | --- | --- |
| Find a court | Home and Explore; sport filter, optional nearby location, paginated cards | `/courts`; responsive, loading, error/retry, empty and saved-state browser tests |
| Book a slot | Court detail; Vietnam calendar date, available future slots, explicit confirmation | `/courts/:id/slots`, book/cancel; fixture flow and actual preview database smoke |
| Create a game | Booking confirmation or retained personal booking; players and description | `POST /games`; validates the returned detail before navigation |
| Find/join a game | Games list and authenticated detail; members, owner controls | `/games`, join/leave/status/cancel; browser workflows and server integration tests |
| Pay a share | Authoritative bill, provider selection, explicit checkout link | Initiation and server status refresh; browser provider fixture only, real PSP acceptance remains open |
| Recover personal activity | Profile bookings and games, pagination, booking-to-game navigation | `/auth/me/bookings`, `/auth/me/games`; actual authenticated preview smoke |
| Manage identity | Login, email OTP registration, reset password, profile and logout | Browser flows plus real API tests; browser credentials remain in memory |
| Change avatar | Photo picker, size/type validation, signed PUT, completion, assignment, removal | Upload/complete/profile fixture tests; actual storage and native picker acceptance remain open |

## State rules

- Validate API DTOs before rendering nested data. A malformed response produces
  an actionable message; background refresh preserves the last valid same-resource data.
- Reset local action state when the route/account identity changes. Invalidate
  asynchronous completions after route blur or unmount, and reset busy state on focus.
- Refresh game detail on focus and every 15 seconds while visible/active. Browser
  hidden tabs and native background states skip polling. This client currently
  uses polling rather than a WebSocket connection.
- Do not infer payment success from opening a provider link. Hide expired links,
  clear stale attempts after provider/bill/member changes, and surface disputed
  payment reconciliation. The backend remains authoritative.
- Use an allowlist for authenticated return navigation. Normalize expected app
  paths; reject auth loops, external, encoded and traversal-like destinations.
- Use the latest stored token pair when updating user metadata. Logout clears
  local credentials immediately and reports an unconfirmed remote revocation.

## Evidence and remaining acceptance

See `artifacts/mobile-validation/summary.md`, `docs/mobile-readiness.md`, and
`artifacts/fullstack/summary.md` for exact checks, commands and limitations.
Browser tests cover 320/390/768/1440 CSS-pixel layouts and enlarged-text pressure.
A Hermes Android export and native config introspection validate bundling and
plugin configuration, not an installed APK.

On physical Android devices, verify system font scaling, TalkBack, keyboard and
gesture-navigation insets, denied location/photo permissions, signed checkout
return navigation, offline interruptions and native secure storage. A production
application identifier, signing setup and installed-device evidence are still
required. Real SMTP, storage and PSP acceptance plus dependency advisory
remediation are tracked in the release-readiness documents.
