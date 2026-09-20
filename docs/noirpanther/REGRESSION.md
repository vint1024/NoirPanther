# Regression checklist

Two passes exist. **Smoke** is what you run after an ordinary change; **full** is what you run
when Stump's version goes up, because an upstream merge rewrites files our features live in and
the failure mode is silence — the code stays, nothing calls it.

Nothing here replaces the automated tests; it covers what they cannot see: layout, readers, the
installed web app, and the mobile clients.

---

## 0. Automated first (always)

```bash
# server: 103 integration + 550 unit tests, incl. tests/fork (our features) and tests/security
cargo test --workspace -- --test-threads=1
cargo fmt --all --check && cargo clippy -p stump_server

# web: 289 tests
yarn workspace @stump/browser test
cd packages/browser && npx tsc -b tsconfig.json
npx eslint packages/browser/src packages/components/src --quiet

# the client's GraphQL documents against the server schema, and against a running server
cd ../../../noirpanther
node scripts/compat/check-schema.cjs                       # 63 documents, 7 fork-only
node scripts/compat/live.cjs http://localhost:10912 cat cattest12345    # 64/64
```

After a merge also run, against a server started from the merged code:

```bash
python3 scripts/noirpanther/authz_check.py http://localhost:10912 \
  --owner cat:cattest12345 --user dog:dogtest12345 --outsider panther_m0:memberpass123
python3 .build-logs/chunk_cycles.py apps/web/dist/assets    # expect "cycles 0"
```

> `chunk_cycles.py` takes the **assets** directory. Pointed at `dist` it silently reports one
> chunk and no cycles, which tells you nothing.

---

## 1. Web (browser, against `:10912`)

Smoke = ★. Everything else is for a version bump.

| #   | What                                         | Looking for                                                                             |
| --- | -------------------------------------------- | --------------------------------------------------------------------------------------- |
| ★1  | Open `/auth`                                 | Neon wordmark, panther, "Запомнить меня", Russian labels                                |
| ★2  | Log in as `cat`                              | Lands on home, covers load, no console errors                                           |
| ★3  | Home                                         | "Продолжить чтение" carousel, recently added series and books                           |
| 4   | Themes → each of the six, incl. a light one  | Whole page switches, sidebar included; no dark-on-dark text                             |
| ★5  | Open an EPUB (Pride and Prejudice)           | Two-column spread with text; page turn works; **no CSP errors in the console**          |
| 6   | EPUB: table of contents, bookmark, font size | Applies without reload                                                                  |
| ★7  | Open a comic (Pepper & Carrot)               | Pages render, slider moves                                                              |
| 8   | Open a manga (Hokusai)                       | Starts right-to-left by itself                                                          |
| 9   | Book page                                    | Description is formatted text, **not** raw HTML tags and not a grey code block          |
| 10  | Series → settings                            | "Объединение" and "Удалить серию" are there and work                                    |
| 11  | Create library                               | "Дополнительные папки" **and** "Папка одиночных книг" both present                      |
| 12  | Settings → Server → General                  | Version reads `0.1.x-rN`, build channel `NoirPanther (stable)`, NoirPanther links first |
| 13  | Settings → Users                             | Create a user, set a content rule, log in as them: the ruled-out book is gone           |
| 14  | Search: `толстой`, `ВОЙНА`                   | Finds books by author and regardless of case                                            |
| 15  | Jobs, Logs, Metadata screens                 | Russian throughout, no raw `settingsScene.…` keys                                       |

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

## 3. Mobile app (iOS / iPadOS / Android)

Devices: iPhone 17 Pro (iOS 26+), iPad Pro (iPadOS 26+), Pixel 6 Pro emulator. For a release also
iOS 27 and the Mac build from the dmg.

| #   | What                                                    | Looking for                                                                                   |
| --- | ------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| ★1  | Launch, log in                                          | Catalog loads                                                                                 |
| ★2  | Open an EPUB                                            | Text renders; edge taps turn pages; chapter title does not overlap the page counter (Android) |
| ★3  | Open a comic                                            | Pages render; manga opens right-to-left                                                       |
| 4   | Book card                                               | Description is formatted text with working links                                              |
| 5   | Download a book, then airplane mode                     | Opens offline, at the page you left it                                                        |
| 6   | Read offline, back online                               | Progress syncs without a false conflict prompt                                                |
| 7   | Deep link `noirpanther://reader/<id>` from a cold start | Book opens, not a blank page                                                                  |
| 8   | Stump compatibility toggle on, against a vanilla server | Fork-only features hidden, catalog still works                                                |

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
