# PR Reviews

Pre-PR code review notes. Drop one file here every time you're about to open a PR — review yourself before review by others.

## Convention

- Filename: `YYYY-MM-DD-<short-slug>.md` (e.g. `2026-04-30-server-foundation.md`).
  - Date = day the review was written, not commit date.
  - Slug = ≤ 4 words, kebab-case, describes the scope (e.g. `auth-rate-limit`, `bill-split-fix`).
- Copy [`_TEMPLATE.md`](./_TEMPLATE.md) and fill in the sections. Sections marked _optional_ may be omitted.
- One review = one upcoming PR. If a single branch grows into multiple PRs, write one review per PR.
- Keep the file even after the PR ships — they double as a changelog of what we knew at merge time.

## Severity legend

| Tag | Meaning | Must fix before merge? |
| --- | --- | --- |
| `[blocker]` | Bug, security hole, broken build, data loss risk | Yes |
| `[high]` | Wrong behavior under realistic conditions, misleading code | Strongly preferred |
| `[medium]` | Will bite us later — leaky abstraction, missing test, unclear API | Open follow-up issue |
| `[low]` | Style, naming, minor cleanup | Optional |
| `[note]` | FYI, no action needed | No |

## Workflow

1. Create the review file from the template.
2. Walk every changed file in the diff (`git diff main...HEAD --name-only`) and record findings.
3. Fix `[blocker]` and `[high]` items in the same branch.
4. Move `[medium]` items to follow-up issues if not addressed in this PR — link the issue in the review.
5. Reference the review in the PR description: `Self-review: docs/pr-reviews/2026-04-30-server-foundation.md`.
