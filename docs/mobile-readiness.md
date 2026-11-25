# Mobile verification and release readiness

Use Node 22.23.3 (`mobile/.nvmrc`) and npm 11.19.0. Run the following in `mobile`:

```sh
npm ci
npm run check:expo
npm run doctor
npm run typecheck
npm test
npm run export:web
npm run export:android
npx playwright install chromium
npm run test:e2e
npm run test:e2e:static
npm audit

# Optional lightweight static web view after export:web
PORT=8082 npm run serve:web
```

The lockfile is required. Expo SDK 57's bundled native module manifest specifies React Native Reanimated 4.5.1 and Worklets 0.10.1; both are explicitly pinned because automatic optional peer installation had selected incompatible 4.9/0.13 releases. Expo's compatibility check and all 21 doctor checks pass with the pinned versions. Expo Doctor is itself pinned to 1.20.4 for reproducibility. The Expo packages are aligned to the SDK 57 patch set (`expo` 57.0.34, Router 57.0.32, ImagePicker 57.0.22, Linking 57.0.14, Location 57.0.24, SplashScreen 57.0.12). Vite is an explicit dev dependency because Vitest requires it at runtime.

The Mobile workflow checks dependency compatibility, TypeScript, unit tests, the production web export, Chromium browser tests at phone and desktop sizes, and an exported-site deep-link smoke test. Browser tests exercise an isolated local API fixture; they do not establish production backend availability. The test server defaults to 18081 and the fixture API to 18080 so unrelated local services on 8080/8081 are not reused. CI uploads browser traces/screenshots on failure and the dependency audit report. Its audit step produces a visible warning for current upstream advisories; a green verification job does **not** mean the dependency audit is clean.

`e2e/fullstack.spec.ts` additionally exercises the actual disposable preview backend without request mocking. It opts in with `PICKUP_FULLSTACK=1` and runs only in the mobile Chromium project to avoid concurrent mutations of one preview account. With the preview runtime described in `artifacts/fullstack/summary.md` active, run `PICKUP_FULLSTACK=1 PICKUP_API_URL=http://127.0.0.1:18080 PICKUP_WEB_URL=http://127.0.0.1:18081 npx playwright test fullstack.spec.ts --project=mobile-chromium --workers=1` (PowerShell: set the three environment variables first). It checks readiness, login, booking, game creation, authenticated history recovery, and cancellation cleanup, and attaches evidence without tokens. The synthetic preview login is restricted to the local loopback API in this test. This validates the app/API/database integration; preview payment and storage providers remain unavailable.

The photo picker uses `expo-image-picker` 57.0.22, matching both the SDK 57 bundled module manifest and the [official ImagePicker recommendation](https://docs.expo.dev/versions/latest/sdk/imagepicker/). Native permission configuration changes require rebuilding the app binary; web checks do not verify native photo permission prompts.

## Android verification — 2026-10-05

`npx expo export --platform android --output-dir ../artifacts/mobile-android-export` successfully produces a Hermes production JavaScript bundle. `npx expo config --type introspect` resolves native plugins, includes location permissions, and marks camera/microphone permissions for removal in the Android manifest because the avatar picker selects existing photos. Generated exports/configuration are ignored build artifacts.

Read-only local inspection found no Java, adb, emulator, or sdkmanager on PATH, no configured JAVA_HOME/ANDROID_HOME/ANDROID_SDK_ROOT, and no Android SDK or Android Studio in the standard local installation paths. No emulator process was present. An APK build or physical-device validation has therefore **not** been performed. No SDK was installed, signing keys generated, or production application identifier invented. Expo introspection substitutes `com.placeholder.appid` internally; this is not a release identifier.

Browser checks cover 320/390/768/1440 CSS-pixel layouts and approximate 130% text enlargement at phone widths. Physical Android checks still need the keyboard open on login/registration/profile, system font scaling, gesture/navigation-bar safe areas, permission denial, secure storage, and photo picking. Browser emulation cannot establish those native behaviors.

## Dependency security review — 2026-11-25

After aligning the Expo SDK 57 patch set, `npm audit` reports six affected package entries (three high, three moderate), propagated from `decode-uri-component`/`query-string` and `minimatch`/`@expo/fingerprint`. Do not run `npm audit fix --force`: npm's suggested fix downgrades Expo to 52 or upgrades Router to 58, both outside the current SDK compatibility set. These remain release-owner review items; a green build does not accept the risk.

| Advisory root | Current constraint / disposition |
| --- | --- |
| [decode-uri-component](https://github.com/advisories/GHSA-vcc3-ghjq-m6fr) and [quadratic decoding](https://github.com/advisories/GHSA-qr28-fpjm-wq7f) | Flows through `query-string` into `expo-router`. npm's compatible fix requires Router 58, outside the SDK 57 set. Do not override the package blindly; re-evaluate reachability and upgrade through a supported Expo release. |
| [minimatch](https://github.com/advisories/GHSA-g3hv-75wg-m249) | Flows through `@expo/fingerprint` into Expo. npm's suggested fix downgrades Expo to 52. Keep build/development servers private and upgrade through a supported Expo release. |

These findings require release-owner review and upstream remediation; this document does not accept the security risk on the owner's behalf. Keep build/development servers private, use trusted build inputs, and re-run the audit before a release. Do not classify all transitive advisories as build-only without checking the exported runtime bundle.

## Deployment checks still required

- Set `EXPO_PUBLIC_API_URL` to the intended HTTPS API endpoint before exporting/building. Values prefixed `EXPO_PUBLIC_` are public bundle configuration and must contain no credentials.
- Exercise the app against the deployed backend, including expired sessions, authorization failures, interrupted requests, and payment redirects.
- Produce and install Android/iOS release builds on physical devices. A web export and Expo Doctor do not verify native signing, location permission behavior, secure storage, or store submission requirements.
- Review privacy disclosures, real production payment/email/storage configuration, monitoring, backups, and the dependency findings above before publishing.
