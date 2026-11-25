# Mobile validation — 2026-11-25

Final verification ran after the static web server, API configuration, session/bookmark resilience, web sharing, accessibility, sport fallback artwork, split desktop auth layout, profile activity panel, route-aware browser titles, optimized WebP hero, and Expo SDK 57 patch-alignment changes.

| Check | Result |
| --- | --- |
| Locked installation | `npm ci` passed with Node 22.23.3 / npm 11.19.0. |
| Browser suite | **90 passed, 2 intentionally skipped**, 92 cases total. The duplicate desktop full-stack case skips to avoid concurrent mutation of one disposable preview account. |
| Real API full-stack journey | **1 passed** with no API route mocking: readiness, login, booking, game creation, authenticated history and cancellation cleanup. |
| Exported-site deep link | **1 passed** against the production web export. |
| Unit tests | **49 passed** across six test files. |
| TypeScript | `npm run typecheck` passed. |
| Expo dependency alignment | `npm run check:expo` passed; dependencies up to date. |
| Expo Doctor | **21/21 passed**. |
| Production web export | Passed; approximately **1.3 MB** JavaScript bundle and a **230 KB** WebP hero (replacing the 2.5 MB PNG). |
| Android production JavaScript export | Passed; **1321 modules, 33 assets**, approximately **2.9 MB** Hermes bundle. |
| Native plugin introspection | Passed; Android activity uses `adjustResize`, branded splash theme resolves, camera/microphone permissions are marked for removal, and location permissions are included. |
| Dependency security audit | **6 affected package entries: 3 high, 3 moderate**; `decode-uri-component`/`query-string` and `minimatch`/`@expo/fingerprint` remain. See `../../docs/mobile-readiness.md` and `../mobile-audit.json`. |

Browser coverage includes discovery, filtering/navigation, saved-court persistence, login and protected return routing, email/OTP registration, password reset, logout/revocation failure, booking and cancellation, game creation/start/completion/join/leave, bill display, payment initiation/provider popup/status refresh/expired links, signed avatar upload/completion/profile update/removal, and oversized-photo rejection. Layout checks cover 320/390/768/1440 CSS pixels. Discovery/profile/court/game controls also pass approximate 130% text enlargement at phone widths; a 50-character account name passes phone and desktop layout checks. These browser checks do not prove native OS font scaling or keyboard behavior.

The actual preview smoke uses **no API route mocking**. It verifies `/health/ready` 200, actual HTTP login, a future court booking, game creation, booking/game retrieval through the authenticated profile, and UI cancellation cleanup. Its synthetic account and disposable PostgreSQL/Redis preview are documented in `../fullstack/summary.md`. Payment/storage providers remain intentionally unavailable in this preview, so provider acceptance and real avatar storage are covered only by controlled mocks and remain release checks.

Actual homepage screenshots (real preview API):

- `../mobile-qa/actual-home-390.png`
- `../mobile-qa/actual-home-1440.png`
- `../mobile-qa/fullstack-history.png`

Additional screenshots cover narrow layouts and enlarged text. Safe command logs are in this directory (`browser.log`, `test.log`, `typecheck.log`, `check-expo.log`, `doctor.log`, `export-web.log`, `export-android.log`). Generated PNGs, web/Android exports, native config JSON, browser reports and trace artifacts are ignored. No source files or this summary are hidden by those exclusions.

Local native tooling inspection found no Java/adb/emulator/sdkmanager on PATH, no configured Android/Java home variables, no standard Android SDK or Android Studio installation, and no emulator process. An APK build, device installation, Android keyboard/safe-area/font-scale testing, signing, and store submission are therefore **unverified**. No SDK was installed, production application identifier invented, or signing key generated. Expo introspection's internal `com.placeholder.appid` is not a release identifier.
