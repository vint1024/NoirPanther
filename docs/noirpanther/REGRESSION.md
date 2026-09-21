# Regression checklist

Two passes exist. **Smoke** is what you run after an ordinary change; **full** is what you run
when Stump's version goes up, because an upstream merge rewrites files our features live in and
the failure mode is silence — the code stays, nothing calls it.

Nothing here replaces the automated tests; it covers what they cannot see: layout, readers, the
installed web app, and the mobile clients.

---

> 🔴 Everything in this document runs against the **local stand** (`:10912`) unless a row says
> otherwise. Production (`:10802`, `:10803`, the demo) is for reading — a version check, a book
> count, logs, or reproducing something the owner reported — never for trying things out.

## 0. Automated first (always)

```bash
# server on SQLite: integration (tests/fork, tests/security) + unit
cargo test --workspace -- --test-threads=1
cargo fmt --all --check && cargo clippy -p stump_server

# server on PostgreSQL — production's backend. Same suite, real migrations, a throwaway
# database per test. Run it before every release and after every upstream merge: SQLite hides
# the differences the fork's pg fixes (A17/A18/A19) exist for.
TEST_DATABASE_URL=postgresql://stump:stump-local@localhost:15432/stump \
  cargo test -p stump_server --test api_tests -- --test-threads=1
# leftovers, if a run was interrupted (they are dropped automatically after 30 minutes):
#   docker exec noir-pg psql -U stump -d stump -tAc \
#     "select datname from pg_database where datname like 'stump_test_%'"

# app: pure logic (page order, description parsing, server timestamps)
cd ../../../noirpanther && yarn test                      # 81 tests

# web: 293 tests
yarn workspace @stump/browser test
cd packages/browser && npx tsc -b tsconfig.json
npx eslint packages/browser/src packages/components/src --quiet

# the client's GraphQL documents against the server schema, and against a running server
cd ../../../noirpanther
node scripts/compat/check-schema.cjs                       # 63 documents, 7 fork-only
node scripts/compat/live.cjs http://localhost:10912 cat cattest12345    # 65/65
```

After a merge also run, against a server started from the merged code:

```bash
python3 scripts/noirpanther/authz_check.py http://localhost:10912 \
  --owner cat:cattest12345 --user dog:dogtest12345 --outsider panther_m0:memberpass123
python3 .build-logs/chunk_cycles.py apps/web/dist/assets    # expect "cycles 0"
```

> `chunk_cycles.py` takes the **assets** directory. Pointed at `dist` it silently reports one
> chunk and no cycles, which tells you nothing.

### What no test covers — do these by hand

Three fork features have no automated test and will not get one cheaply. Each is a few minutes:

| Feature                              | How to check it                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A14** memory bounding              | Watch `docker stats` during a scan that is happening anyway (a release rescan, or a big library on the local stand). RSS should plateau in the hundreds of MB rather than climb until the container is killed; compare with the previous release's figure in `.build-logs/STATUS.md`. Do not start a production rescan just to measure this.                                                                                                                                                                           |
| **A21** a library root that vanished | On `:10912`, rename one of a library's folders on disk, run a scan, and look at the library's books: they must be marked missing, not deleted, and not silently kept as present. Rename the folder back, scan again: they come back. 🔴 The renamed folder also leaves a phantom series behind (the scanner sees a new folder as a new series) — delete it afterwards, or the stand keeps duplicate books. The old scan harness in `core/integration-tests` predates the sea-orm rewrite, which is why this is manual. |
| **A20** versioned thumbnail URLs     | Open a book page in the browser, note the cover URL carries `?last_modified=…`, regenerate the series thumbnail, reload: the value must change and the new cover must show without a hard refresh.                                                                                                                                                                                                                                                                                                                     |

Everything else the fork adds has a test; the map of which test guards which feature is in
`FORK_BACKLOG.md`.

---

## 1. Web (browser, against `:10912`)

Smoke = ★. Everything else is for a version bump.

| #   | What                                         | Looking for                                                                                                                                                                          |
| --- | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| ★1  | Open `/auth`                                 | Neon wordmark, panther, "Запомнить меня", Russian labels                                                                                                                             |
| ★2  | Log in as `cat`                              | Lands on home, covers load, no console errors                                                                                                                                        |
| ★3  | Home                                         | "Продолжить чтение" carousel, recently added series and books                                                                                                                        |
| 4   | Themes → each of the six, incl. a light one  | Whole page switches, sidebar included; no dark-on-dark text                                                                                                                          |
| ★5  | Open an EPUB (Pride and Prejudice)           | Two-column spread with text; page turn works; **no CSP errors in the console**                                                                                                       |
| 6   | EPUB: table of contents, bookmark, font size | Applies without reload                                                                                                                                                               |
| ★7  | Open a comic (Pepper & Carrot)               | Pages render, slider moves                                                                                                                                                           |
| 8   | Open a manga (Hokusai)                       | Starts right-to-left by itself                                                                                                                                                       |
| 9   | Book page                                    | Description is formatted text, **not** raw HTML tags and not a grey code block                                                                                                       |
| 10  | Series → settings                            | "Объединение" and "Удалить серию" are there and work                                                                                                                                 |
| 11  | Create library                               | "Дополнительные папки" **and** "Папка одиночных книг" both present                                                                                                                   |
| 12  | Settings → Server → General                  | Version reads `0.1.x-rN`, build channel `NoirPanther (stable)`, NoirPanther links first                                                                                              |
| 13  | Settings → Users                             | Create a user, set a content rule, log in as them: the ruled-out book is gone                                                                                                        |
| 14  | Search: `толстой`, `ДЕТСТВО`                 | Finds by author (the stand has three Tolstoy books) and regardless of case. 🔴 Pick terms that exist in THIS library — the stand has no «Война и мир», so a miss there means nothing |
| 15  | Jobs, Logs, Metadata screens                 | Russian throughout, no raw `settingsScene.…` keys                                                                                                                                    |

**Layout check on the screens upstream rewrote**: server stats, jobs table, metadata table, the
translation notice, library settings. Russian strings are longer than English — look for text
sitting on top of buttons.

## 2. Installed web app (PWA)

| #   | What                                       | Looking for                                             |
| --- | ------------------------------------------ | ------------------------------------------------------- |
| ★1  | `curl -sI <server>/manifest.webmanifest`   | `application/manifest+json`, not HTML                   |
| 2   | Safari on iOS → Share → "На экран «Домой»" | Panther icon, name "NoirPanther" (not the page title)   |
| 3   | Open from the home screen                  | No browser chrome; login remembers you (30-day session) |
| 4   | Mobile width in the browser (375px)        | Search box on the home screen, two-row filter bar       |
| 5   | Navigate deep, then back                   | Lists return to the position you left them at           |

## 3. Mobile app (iOS / iPadOS / Android / Mac)

**Before you touch any device, three checks that cost a minute and save an evening:**

1. **Android: is the installed app a debug build?** A release APK carries its own JS bundle and
   never talks to Metro, so nothing you edit shows up and you end up testing last month's code.
   ```bash
   adb shell dumpsys package in.kuvshinov.noirpanther | grep -E "flags=|versionName"
   ```
   `flags=0x0` with no `DEBUGGABLE` means release. Install a debug build instead:
   `cd android && JAVA_HOME=$JDK17 ./gradlew --no-daemon :app:assembleDebug` then
   `adb install -r app/build/outputs/apk/debug/app-debug.apk`.
2. **Mac: bring the window to the front first** (`osascript -e 'tell application id "in.kuvshinov.noirpanther" to activate'`).
   An inactive Catalyst window draws no toolbar items and takes no synthesised clicks — both look
   exactly like a broken build.
3. **Native header items and tab bars do not survive Fast Refresh.** Judge them only after a clean
   app restart.

Also: do not run the Android emulator while an Xcode build is going. The emulator loses its CPU
thread and dies ("hanging thread QEMU2 CPU0"), or wedges with an ANR.

**Devices and how to drive them** (UDIDs and the rest: memory `environment-and-test-creds`):

| Device                  | UDID                                   | When                           |
| ----------------------- | -------------------------------------- | ------------------------------ |
| iPhone 17 Pro, iOS 26.5 | `42941A3A-5B89-499D-AFBC-38B3279E5803` | every pass                     |
| iPad Pro 11" (M5), 26.5 | `6D605BA2-F185-4123-A6C3-83331298544E` | every pass (Liquid Glass)      |
| iPhone 17 Pro, iOS 27.0 | `1B5796B3-CD7E-4DD3-86B6-3F15E5B99CC7` | before a release               |
| iPad Pro 11" (M5), 27.0 | `7D01F8B5-7887-4BE9-87F3-82931B73F5EE` | before a release               |
| Pixel 6 Pro (API 34)    | AVD `Pixel_6_Pro`                      | every pass; host is `10.0.2.2` |
| Mac (Catalyst)          | the notarized dmg                      | before a release               |

```bash
xcrun simctl boot <udid>                       # "Invalid argument" → killall -9 com.apple.CoreSimulator.CoreSimulatorService
xcrun simctl install <udid> <path>/NoirPanther.app
xcrun simctl launch <udid> in.kuvshinov.noirpanther
xcrun simctl io <udid> screenshot /tmp/x.png   # sips -Z 1000 before reading it
xcrun simctl openurl <udid> "noirpanther://reader/<id>"   # iOS 27 asks "open in app?" — needs a tap
xcrun simctl shutdown all                      # switch simulators off when done
```

Taps, swipes and typing go through the iOS Simulator tool (headless, it does not touch the user's
screen). Each new device needs the user to approve it once in the panel — ask before a pass that
uses one. The app panel is what the user watches: attach it first, then build.

**Smoke (★) — after any app change:**

| #   | What                 | Looking for                                                    |
| --- | -------------------- | -------------------------------------------------------------- |
| ★1  | Launch, log in       | Catalog loads, no red-box warning                              |
| ★2  | Open an EPUB         | Text renders; edge taps turn pages; swipe ≠ tap                |
| ★3  | Open a comic         | Pages render; a manga opens right-to-left                      |
| ★4  | Back out of a reader | The book page still works; the catalog has not reloaded to top |

**Full pass — before a release, and after a Stump version bump:**

_Catalog and navigation_

| #   | What                                    | Looking for                                                      |
| --- | --------------------------------------- | ---------------------------------------------------------------- |
| 1   | All four tabs                           | Titles are Russian, the tab bar hides only inside a club chat    |
| 2   | Books list vs Series list, side by side | Same header style (both large or both small — see the open item) |
| 3   | Book card                               | Description is formatted text with working links, not raw HTML   |
| 4   | Cover tap                               | Lightbox opens, pinch-zooms, closes                              |
| 5   | Search: `толстой`, `ВОЙНА`              | Finds by author and regardless of case                           |
| 6   | Filter and sort sheet                   | Applies, and the list keeps its position afterwards              |

_Readers_

| #   | What                                | Looking for                                                           |
| --- | ----------------------------------- | --------------------------------------------------------------------- |
| 7   | EPUB: taps at the edges, middle tap | Pages turn; the middle tap toggles the chrome, a swipe never does     |
| 8   | EPUB on Android                     | Chapter left, timer right, in line with Readium's own page counter    |
| 9   | EPUB: bookmark, note on a selection | Both save; the note dialog is the cross-platform prompt, not iOS-only |
| 10  | Comic: slider, zoom, direction      | Manga starts RTL; zoom does not fight the page turn                   |
| 11  | Progress: read, leave, come back    | Resumes at the same page, in both readers                             |
| 12  | Incognito on                        | Neither progress nor the reading timer moves                          |

_Offline (E3)_

| #   | What                                             | Looking for                                                                                                                                                                                                                    |
| --- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 13  | Download an EPUB and a CBZ, then airplane mode   | Both open; the comic pages come from the archive on the device                                                                                                                                                                 |
| 14  | Read offline, then go back online                | Progress syncs, **no** false conflict prompt                                                                                                                                                                                   |
| 15  | Downloads screen                                 | Sizes add up; deleting frees the file                                                                                                                                                                                          |
| 16  | An account with OFFLINE_READ but no DownloadFile | Can still take a book offline. Make that account on the LOCAL stand — never test against production                                                                                                                            |
| 17  | Two accounts, one device                         | Download under `cat`, log out, log in as `test`: Downloads is EMPTY and `noirpanther://reader/<that book id>` does NOT open it. Back as `cat`: everything is there, at the same page. A download is private to whoever made it |
| 18  | Stop the server, cold-start the app              | The owner still sees and reads their downloads (the cached user keeps them). Log OUT and they are hidden — that is the rule, not a bug                                                                                         |

_Clubs_

| #   | What                                  | Looking for                                                            |
| --- | ------------------------------------- | ---------------------------------------------------------------------- |
| 19  | Open a discussion, scroll back a page | No message appears twice (this regressed once — `flattenMessagePages`) |
| 20  | Send, edit, delete, react             | All four land; the composer keeps the draft if sending fails           |
| 21  | Open a thread from a message          | Root message on top, replies below, same rules as above                |

_Screens that are easy to forget_ (every one of these was missed on the 2026-09-21 pass and then
found to have changed — they are reached through a menu or a rarely-used path, so a "click around
the app" sweep never lands on them)

| #   | What                                              | Looking for                                                                                                                                                                                                                                                                                                                                                               |
| --- | ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 22  | Club with no current book → "Add a book", search  | Cover and title on ONE row with spacing, not stacked                                                                                                                                                                                                                                                                                                                      |
| 23  | Series → edit → "Merge series", type a name       | Candidates are a bordered card of padded rows, each with its own background                                                                                                                                                                                                                                                                                               |
| 24  | Book → edit metadata, scroll to the tag fields    | Every field has its "+" button; genre chips carry a ✕                                                                                                                                                                                                                                                                                                                     |
| 25  | Browse → Files, two levels down                   | Breadcrumb reads "Root › Folder"; rows keep their padding                                                                                                                                                                                                                                                                                                                 |
| 26  | Book → "..." menu                                 | Every entry translated; "Go to series" lands on the series                                                                                                                                                                                                                                                                                                                |
| 27  | Browse → OPDS catalogs (with none added)          | The add form and the empty state, both translated                                                                                                                                                                                                                                                                                                                         |
| 28  | Reader → contents/bookmarks sheet                 | Rows are full height, the bin button is reachable                                                                                                                                                                                                                                                                                                                         |
| 29  | Settings → server row → pencil → Save             | Saves and returns. On Android this killed the process outright until 2026-09-21 (a store write during the pop) — check logcat has no `addViewAt`. Do it twice: with a changed name and with nothing changed                                                                                                                                                               |
| 30  | Settings → the same form → switch Login ↔ API key | The session re-authenticates (that is what `authKey` is for) and the catalog still loads; switching back restores the login form                                                                                                                                                                                                                                          |
| 31  | Search for a title as it is SHOWN on screen       | e.g. `sunday` finds "Sunday pages 1906", whose file is named "Little Nemo…". Search reads the metadata title, not just the file name                                                                                                                                                                                                                                      |
| 32  | Settings → Data usage                             | Totals count only the signed-in person's downloads, and "clear" frees their files                                                                                                                                                                                                                                                                                         |
| 33  | Downloaded → swipe a row right-to-left            | The Delete button appears and the book does NOT open. Until 2026-09-21 the same gesture did both: a Pressable measures a press by where the finger LIFTS, and a swipe across a full-width row lifts inside it. Long press must open the action sheet (whose item asks the question), a plain tap must still open the book, and a tap on an already-open row must close it |
| 37  | Downloaded → long press a row, on Mac too         | The action sheet opens with the book's name on it. On Mac it is a side panel: it must clear the window toolbar and carry that name in its header — a copy mounted inside the row instead of the screen drew itself INSIDE the row, cut off. Also watch the row while swiping: it must stay opaque, or the red Delete button shows through it                              |
| 34  | Replace a book's file on the server, rescan       | The Downloaded list fetches the new copy by itself (cover, name and size all move) and marks the row "Updated", with the reading position untouched and no sync-conflict screen. The cover is the point: the list draws the copy stored with the download, not the catalog's                                                                                              |
| 35  | Delete a book from the server, rescan             | The file goes from the device (check the size in Settings → Data usage), the row stays marked "No longer on the server", and tapping it explains rather than failing to open. Put the file back and rescan: it downloads again, progress still attached                                                                                                                   |
| 36  | Stop the server, then open Downloaded and pull    | NOTHING is marked missing and no file is deleted — "the book is gone" and "nobody answered" must never be confused. No error banner either: no server is the normal state for this screen                                                                                                                                                                                 |

_Platform specifics_

| #   | What                                       | Looking for                                                                               |
| --- | ------------------------------------------ | ----------------------------------------------------------------------------------------- |
| 20  | iPad: wide layout                          | Bubbles and lists pull in from the edges, nothing hugs the bezel                          |
| 21  | Mac: the header buttons                    | Every one of them reacts to a **mouse** click (custom views do not — they must be native) |
| 22  | Mac: window resize                         | Master/detail in Browse reflows, no clipped text                                          |
| 23  | Android: back gesture out of a reader      | Leaves the reader, not the app                                                            |
| 24  | Dark app theme while the OS is light       | Native chrome (tab bar, headers) stays dark                                               |
| 25  | Deep link `noirpanther://reader/<id>` cold | Opens the book, not a blank page                                                          |
| 26  | Stump compatibility toggle on              | Clubs tab gone, merge section gone, catalog still works                                   |

**Known open items** — none. All three that stood here were closed on 2026-09-21 and are kept
below so nobody re-opens them from memory:

- The two club chat screens shared ~70 % of their code. The composer is now one component
  (`components/bookclubs/ChatComposer`) and the page flattening is shared, which is what the
  duplicate-message bug actually rode on; the screens are down from 438/357 to 310/264 lines.
- The large header title of the list screens is now one definition (`lib/nav/screenOptions`,
  `listScreenOptions`), so books / series / downloads cannot drift apart again.
- Android with the reading timer on: looked at, and the footer is right — chapter left,
  Readium's own counter in the middle, timer on the right, one line, nothing overlapping.

## 4. Servers after deploying

```bash
curl -s -m 10 -X POST <server>/api/v2/version            # version and build channel
curl -sI <server>/manifest.webmanifest                    # application/manifest+json
curl -sI <server>/ | grep -i content-security-policy      # policy present
docker logs <container> --since 5m | grep -iE "error|panic|migrat"
```

Then, in a browser: log in, open a book, open a comic. On the home servers also check the book
count against what it was before the deploy.

---

## Version bump: the order that works

1. Merge, resolve, `cargo check` — then **everything in section 0**.
2. Walk `FORK_BACKLOG.md` feature by feature. For each, ask "does the scenario still work?", not
   "does the symbol still exist" — that distinction is what let content rules stay broken for
   eight days.
3. Web pass (section 1) against a locally built image, **with the browser console open**. The CSP,
   the chunking and the readers break here, not in the tests.
4. App: `check-schema.cjs` against the new schema, then the smoke rows of section 3.
5. Release, deploy to the demo, then the home servers, then re-check section 4.
