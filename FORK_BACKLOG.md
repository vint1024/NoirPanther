# NoirPanther fork — feature backlog & audit checklist

Master list of EVERYTHING our fork added on top of upstream Stump, derived from
the fork commit log (`git log 1ed0f99a..4ebe18a6`) + README + memory. Used as a
regression-test backlog after every upstream merge (the v0.1.5 merge silently
reverted several of these because it took `theirs` for ~97 web files + locales).

Status legend: ✅ present/verified · ⚠️ partially lost · ❌ lost/regressed · ⏳ to-audit

## A. Server / backend (Rust)
| # | Feature | Key symbols / files | Status |
|---|---------|---------------------|--------|
| A1 | Multi-folder libraries (extra paths) | `library_path` entity, migration `library_extra_paths`, `library_scan_job` multi-root loop, `extraPaths` form fields | ✅ backend OK; extraPaths form UI was orphaned — RESTORED this session (field array + picker, both forms) |
| A2 | Reversible series merging | `series_merge`+`series_merges` table, `mergeSeries`/`unmergeSeries` mutations, `fetch_map_for_library` | ✅ backend OK; UI = B10 (RESTORED) |
| A3 | Per-user content access rules (tag/genre/publisher, Unicode-folded) | `content_access_rule` entity+migration, `ContentRule*` graphql, `ContentAccessRulesEditor.tsx` | ✅ audited — `ContentAccessRulesEditor.tsx` wired in `CreateOrUpdateUserForm.tsx` |
| A4 | Series visibility (hide series when all books hidden) + per-user book counts | series object content-rule filtering | ✅ audited — `content_access_rule` entity present on media/series/library/user (Rust untouched by merge) |
| A5 | Metadata writeback into EPUB files (+ backup flag + cleanup) | `metadata_writeback` job, `WritebackScanGate`, librarySettings danger-zone writeback section | ✅ audited — writeback in `mutation/media.rs` + danger-zone `DeletionScene.tsx` |
| A6 | EPUB streaming for read-only users (read-gated /epub/{id}/file + manifest) | resource endpoints, ReadiumManifestGenerator | ✅ audited — `api/v2/epub.rs` `/manifest.json` + `/file` (read-gated) |
| A7 | Server-side EPUB cover placeholder + WebP/GIF/SVG thumbnails | placeholder cover generation | ✅ audited — `filesystem/common.rs` + `content_type.rs` (webp) |
| A8 | Per-series thumbnail regeneration (+ regenerate-from-cover button) | `SeriesThumbnailSelector`, regenerate mutation | ✅ audited — `SeriesThumbnailSelector.tsx` wired in `SeriesSettingsScene` |
| A9 | Offline reading w/ encryption E3 (/offline + device key + OfflineRead cap) | `device_public_keys` migration, `/offline` endpoint, `OfflineRead` permission | ✅ audited — `device_public_key.rs` entity + migration + `OfflineRead` enum |
| A10 | Book clubs at scale (cursor-paginated members/books/discussions, keyset, DataLoader) | `bookClubMembers`/`bookClubPreviousBooks`/`bookClubDiscussionsPaginated` | ✅ audited — `book_club_discussions_paginated` in `query/book_club_discussion.rs` |
| A11 | Unicode case-insensitive search (ulower) | `models::db::register_unicode_functions`, `ulower` in `apply_string_filter` | ✅ audited — `ulower` registered in `models/db.rs`, used in `graphql/filter/mod.rs` |
| A12 | Search by AUTHOR (writers) — `_or` name/writers | media filter writers `_or` | ✅ audited — `writers: StringLikeFilter` in `graphql/filter/media_metadata.rs` |
| A13 | Session sliding expiry (web isn't logged out every TTL) | touch_expiry throttled, OnSessionEnd cookie | ✅ audited — `middleware/auth.rs` + `config/session/store.rs` |
| A14 | Memory bounding (jemalloc + glibc arena/trim + blocking pool 128 + scanner parallelism) | main.rs MAX_BLOCKING_THREADS, Dockerfile MALLOC_* | ✅ audited — `MAX_BLOCKING_THREADS=128` in `main.rs`, `MALLOC_*` in Dockerfile |
| A15 | Hide metadata field lock button without EditMetadata | gated lock icon | ✅ audited — `metadataEditor/cells/LockFieldButton.tsx` |
| A16 | Single-series deletion + content-rule UX polish | per-series delete | ✅ backend OK; delete UI was orphaned — RESTORED this session (mutation + button + ConfirmationModal) |
| A18 | PostgreSQL: book-club `role` columns TEXT on pg (`m20260911_000000_pg_book_club_role_text`), copier `scripts/db/sqlite_to_postgres.py` | migration + script | ✅ added 2026-09-11 (servers migrated to Postgres) |
| A17 | `ulower()` on PostgreSQL (SQL wrapper over `lower()`) so string filters / content rules are backend-agnostic | `core/src/database.rs` postgres branch `CREATE OR REPLACE FUNCTION ulower` | ✅ added in the v0.1.7 merge (2026-09-11) |

## B. Web / UI
| # | Feature | Key symbols / files | Status |
|---|---------|---------------------|--------|
| B1 | Full Russian localization (616 keys / ~165 components → t()) | en-US/ru-RU locales + useLocaleContext in components | ✅ (recovered this session) |
| B2 | Rebrand: name, panther emblem, favicons, splash, PWA manifest | brand strings, assets | ⏳ |
| B3 | Six NoirPanther themes + Vibranium default everywhere | `themes.css` 6 classes, useApplyTheme default vibranium, ThemeSelect list | ✅ (recovered this session) |
| B4 | Vanilla Stump theme (follows OS) | useApplyTheme 'vanilla'→dark/light | ✅ (recovered) |
| B5 | 2-row filter bar on mobile/PWA (search row + controls row) | FilterHeader.tsx | ✅ (recovered this session) |
| B6 | Neon-sign login wordmark (flicker/buzz) + ambient neon wash | LoginOrClaimScene neon-sign, `.n` class | ✅ audited — `LoginOrClaimScene.tsx` neon present (wordmark color fixed `from-primary to-ring`) |
| B7 | Line-art panther favicon + head-only favicon | favicon assets | ✅ audited — favicon/PWA assets present (brand audit B2) |
| B8 | PWA service worker from dist root + 6 MiB precache | vite-plugin-pwa config | ✅ audited — `VitePWA` + `maximumFileSizeToCacheInBytes: 6 MB` in `apps/web/vite.config.ts` |
| B9 | EPUB streaming on web (infinite-spinner fix) | reader resource endpoint usage | ✅ audited — server `api/v2/epub.rs` serves manifest+file (A6) |
| B10 | Series-merge UI rendered in Series settings | `MergeSeriesSection.tsx` wired into `SeriesSettingsScene` | ✅ FIXED — re-imported + rendered (ManageLibrary-gated) this session |
| B11 | Search-by-author in web | media filter writers `_or` (server) | ✅ N/A — not a bug; user searched in Series, book search matches author fine |
| B12 | Folder file-explorer icons | `Folder.png` capital (case-fix) | ✅ (fixed this session) |

## C. Apps (noirpanther — separate repo, not affected by server merge)
Tracked separately; built 0.1.1/build7 (iOS TF + Android + Mac). Not in scope of
this server-merge audit, but listed: GraphQL reading-progress migration, metadata
editing, search-by-author, offline encryption E2/E3, OPDS, book clubs, themes.

### App TODO (not yet done)
- [ ] Unified search across SERIES + BOOKS + AUTHORS in one query (spawn_task task_86802da7). Currently book search matches title+author but there's no combined cross-entity search.

## Audit log
- **v0.1.5 merge (2026-06):** 114 conflicts; regressions found in 3 rounds (codegen theirs, i18n/components theirs, orphaned parent scenes). All items restored.
- **v0.1.7 merge (2026-09-11, merge `08123931`):** 39 conflicts. Upstream took over localisation of the EPUB/image reader controls, thumbnail selectors and error scene with its own keys (theirs taken, ru translated); our `t()` re-applied to users table/menu, filter drawer, fullscreen toggle. EPUB streaming (`readium.rs`/`epub.rs`) = upstream (their PR #1288 grew from our A6); fork-only read-gated `/epub/{id}/file` dropped by decision. A1–A17 + B2–B12 verified present & wired (paths moved: `packages/components/tailwind/themes.css`, `apps/web/vite.config.mts`, `apps/web/src/index.html`). New failure mode seen: **auto-merge duplicates `const { t } = useLocaleContext()`** when both sides add it (BookManagementScene) → tsc catches it; and **upstream component API drift** (CheckBox lost `variant`) in our-only files (MetadataWriteback) → tsc catches it. Always run `yarn --cwd packages/browser check-types` after a merge.

---
**Audit method:** for each ⏳ item, confirm the key symbol/file exists in current
tree AND is wired/rendered (not just present-but-orphaned, like B10). After the
v0.1.5 merge the failure mode is "took theirs" → our addition reverted.
