<p align="center">
  <img src=".github/assets/noirpanther-banner.svg" alt="NoirPanther" width="640" />
</p>

# NoirPanther server

> **NoirPanther server is a fork of [Stump](https://github.com/stumpapp/stump)** — a self-hosted server for comics, manga and digital books.
>
> - Repository: <https://github.com/vint1024/NoirPanther> (development mirror: <https://git.vint1024.net/vint1024/stump.git>)
> - Upstream (original): <https://github.com/stumpapp/stump>
> - Current release: **`0.1.7-r4`** = everything in Stump **0.1.7** + [what this fork adds](#what-this-fork-adds). See [Versioning](#versioning).
>
> **All modifications in this fork were developed with heavy assistance from AI tooling.**

## Install with Docker Compose

Multi-arch images (`linux/amd64`, `linux/arm64`) are published to the GitHub Container
Registry — no login required:

| Tag                                     | What you get                                                    |
| --------------------------------------- | --------------------------------------------------------------- |
| `ghcr.io/vint1024/noirpanther:latest`   | the newest release                                              |
| `ghcr.io/vint1024/noirpanther:0.1.7`    | the newest fork revision built on Stump 0.1.7                   |
| `ghcr.io/vint1024/noirpanther:0.1.7-r4` | exactly this release (pinned — updates only when you change it) |

### With the built-in SQLite database

The simplest setup — one container, the database is a file in the config folder.

```yaml
# docker-compose.yml
services:
  noirpanther:
    image: ghcr.io/vint1024/noirpanther:latest
    container_name: noirpanther
    volumes:
      - ./noirpanther_config:/config # database, thumbnails, avatars, logs
      - /path/to/your/books:/data
    ports:
      - 10801:10801
    environment:
      - PUID=1000
      - PGID=1000
      - TZ=UTC
      - STUMP_CONFIG_DIR=/config
    restart: unless-stopped
```

### With PostgreSQL

Recommended for large libraries (tens of thousands of books) and several simultaneous users.
Put the database password into a `.env` file next to the compose file
(`STUMP_DB_PASSWORD=choose-a-password`).

```yaml
# docker-compose.yml
services:
  db:
    image: postgres:18-alpine
    container_name: noirpanther-db
    environment:
      POSTGRES_USER: stump
      POSTGRES_PASSWORD: ${STUMP_DB_PASSWORD}
      POSTGRES_DB: stump
    volumes:
      - pgdata:/var/lib/postgresql
    healthcheck:
      test: ['CMD-SHELL', 'pg_isready -U stump -d stump']
      interval: 5s
      timeout: 3s
      retries: 30
    restart: unless-stopped

  noirpanther:
    image: ghcr.io/vint1024/noirpanther:latest
    container_name: noirpanther
    depends_on:
      db:
        condition: service_healthy
    volumes:
      - ./noirpanther_config:/config # thumbnails, avatars, logs (the database lives in `db`)
      - /path/to/your/books:/data
    ports:
      - 10801:10801
    environment:
      - PUID=1000
      - PGID=1000
      - TZ=UTC
      - STUMP_CONFIG_DIR=/config
      - STUMP_DB_HOST=db
      - STUMP_DB_PORT=5432
      - STUMP_DB_NAME=stump
      - STUMP_DB_USER=stump
      - STUMP_DB_PASSWORD=${STUMP_DB_PASSWORD}
    restart: unless-stopped

volumes:
  pgdata:
```

### Run it

```bash
docker compose up -d
```

Open `http://<host>:10801` — the first account you create becomes the server owner. Then add a
library pointing at `/data`.

Updating:

```bash
docker compose pull && docker compose up -d
```

- Both files live in [`docker/examples`](docker/examples): [`docker-compose.yml`](docker/examples/docker-compose.yml),
  [`docker-compose.postgres.yml`](docker/examples/docker-compose.postgres.yml), plus
  [`backup-postgres.sh`](docker/examples/backup-postgres.sh) for database dumps.
- Moving an existing SQLite database to PostgreSQL: start the PostgreSQL stack once (it creates the
  schema), stop the server container, run [`scripts/db/sqlite_to_postgres.py`](scripts/db/sqlite_to_postgres.py),
  start it again.
- Any Stump client works with the server (OPDS, KOReader, Kobo and the Stump apps included); the
  NoirPanther app adds the fork-only features on top and keeps a compatibility mode for vanilla Stump servers.

## Versioning

The version is deliberately kept **the same as the Stump release the build is made from**, with
a fork revision after it: **`<Stump version>-r<N>`**.

- `0.1.7-r1` — built from Stump `0.1.7`: **everything Stump 0.1.7 has** (PostgreSQL support, the
  Readium-based web EPUB reader with streaming, whole-book search and annotations, Comic Vine +
  manual metadata search, reading-session conflict resolution, avatars, … and the post-release
  image-reader history fix) **plus everything
  listed under [What this fork adds](#what-this-fork-adds)**.
- `-r2`, `-r3`, … — our own fixes and features on top of the same Stump release.
- When Stump publishes `0.1.8` and it is merged here, the numbering restarts: `0.1.8-r1`.

Each release is a git tag `v<version>` (e.g. `v0.1.7-r4`), a [GitHub release](https://github.com/vint1024/NoirPanther/releases)
with notes, and a Docker image with the same tag. The server checks that releases page itself:
when a newer one exists, **Settings → Server → General** shows a notice with a link to the release
notes. (The check asks the GitHub API from the server; nothing about your server is sent.)

## What this fork adds

### Brand / UI

- **Rebranded to "NoirPanther server"** — name, panther emblem, favicons, splash and PWA manifest
- **Six NoirPanther themes** ported from the NoirPanther mobile client — _Vibranium_ (default), _Golden Eye_, _Emerald Gaze_, _Cinematic Noir_, and the _Vibranium · Light_ / _Golden Eye · Light_ variants (the upstream Stump themes are kept too)
- **Full Russian localization of the web UI** — complete coverage (~2,590 i18n keys, 340+ localized components)
- Server-info screen surfaces the fork identity, our GitHub / release links and the full `<Stump version>-r<N>` version
- **Update notice** — the server checks this repository's GitHub releases and tells the owner when a newer NoirPanther release exists (the upstream check never showed up: the web app asked a route the server doesn't have)

### Server &amp; features

- **EPUB streaming for read-only users** — manifest + per-resource endpoints so users without download permission can read EPUBs in the browser/app (our implementation seeded upstream PR #1288; since v0.1.7 the fork ships the upstream Readium reader on top of it)
- **Offline reading with encryption (E3)** — an `/offline` endpoint that wraps the content key to a device's Secure-Enclave public key, gated by an `OfflineRead` capability (no download permission required)
- **Multiple folders per library**
- **Reversible series merging**
- **Content access rules** by tag / genre / publisher (Unicode case-folded)
- **Series visibility** — hide a series when all of its books are hidden by content rules; per-user book counts
- **Write metadata back into EPUB files** — with an opt-in backup flag and backup cleanup
- **Server-side EPUB cover placeholder** + WebP / GIF / SVG thumbnail support
- **Series thumbnail regeneration** (incl. a regenerate-from-cover button)
- **Book clubs, enabled and at scale** — switched on in production builds (upstream still hides them) with cursor-paginated members / past books / discussions, keyset discussion history, and DataLoader fixes for the member graph
- **PostgreSQL-ready fork**: on Postgres the string-backed book-club role columns are fixed (upstream declares them INTEGER, which breaks clubs there), `ulower()` is provided as a SQL wrapper, and `scripts/db/sqlite_to_postgres.py` copies an existing SQLite database into a freshly migrated PostgreSQL one (used to move this fork's own servers off SQLite); see [Install with Docker Compose](#with-postgresql). The whole API was then audited against PostgreSQL (every client operation, OPDS 1.2/2.0, KOReader, Kobo, scans, content rules, smart lists, book clubs) and the upstream queries that only worked on SQLite were fixed: library scans (`LIKE … ESCAPE` placeholders), metadata / visit / device upserts (`ON CONFLICT` targets), reading-status filters and Kobo sync (`GROUP BY`), OPDS progression (JSON device join), re-favoriting, `series.library` sub-filters
- **Cover lightbox**: click a book cover on its page to view it full-height (Esc, ✕ or a click outside closes it)
- **Security fixes on top of Stump 0.1.7** — privileged account fields (permissions, age restriction, session limit) can only be changed by the server owner — upstream is missing that authorization check; the missing-files listing of a library now needs `MANAGE_LIBRARY`; previous book-club discussions check club access; the metadata overview (genres, authors, publishers…) only covers books the caller can see. `scripts/noirpanther/authz_check.py` re-checks all of it against a test server
- **Manga opens right-to-left by itself**: the `Manga` tag of `ComicInfo.xml` (`YesAndRightToLeft`) becomes a per-book reading direction (upstream only has a per-library default); the web reader and the NoirPanther app start such books RTL, the reader's own setting still overrides it
- **Clean titles and comic languages from file metadata**: EPUBs with several `dc:title` elements (main / subtitle / full title) get the main title instead of a multi-line mash-up; EPUB 3 collections are resolved through `refines`, so only a real `series` collection becomes the series (not the "100 best novels" sets listed beside it); `LanguageISO` from `ComicInfo.xml` is read (upstream only accepts a non-standard `Language` tag)
- **Covers that actually refresh**: thumbnail URLs carry a version (`?v=…`) that changes when a cover is regenerated or a library rescanned, so browsers and the NoirPanther app stop showing stale covers despite the one-year cache header
- **Unicode case-insensitive search** (`ulower`) — search matches regardless of letter case in any language (incl. Cyrillic); **search also matches book authors** (writers), not just titles. Works on both SQLite (custom function) and PostgreSQL (SQL wrapper over `lower()`)
- **Book & series metadata editor** in the web UI — permission-gated (`EditMetadata`), with autocomplete and field-level locks
- **Sliding session expiry** — active web sessions are refreshed instead of logging the user out on a fixed TTL
- **"Remember me"** — the login form can ask for a 30-day persistent session (`?remember=true`), on by default when the web app runs as an installed PWA; the login page also skips itself when a session already exists (an iOS home-screen web app installed from the login page opens on `/auth` every time)
- **Installable as a home-screen web app** — the manifest is actually served (upstream let it fall through to the SPA fallback), so iOS picks up the panther icon and the "NoirPanther" name; the login page hands a signed-in user straight through, and covers don't blink after the edge-swipe back
- **Scroll position restored on Back** — every list (home, book/series search, libraries, series) returns to where you left it when you navigate back
- **Search on mobile / PWA** — a search box at the top of the home page and in the slide-out menu that opens the book search; a `?search=` arriving via link now also shows up in the search field
- **Memory-bounded server** — tuned allocator (jemalloc + glibc arena/trim), bounded blocking pool and scanner concurrency for stable memory on large libraries
- Content rules in the user-creation form; tag / genre / publisher autocomplete; single-series deletion
- Fixes: metadata lock hidden without `EditMetadata` permission; writeback / file-watcher race; cross-origin EPUB reader credentials; N+1 queries; owner-rule handling; `OfflineRead` permission checks

### Packaging

- Version **`0.1.7-r4`** (see [Versioning](#versioning)), build channel **`NoirPanther (stable)`**
- Docker images for `amd64` + `arm64` published to **`ghcr.io/vint1024/noirpanther`** by the [`NoirPanther Docker image`](.github/workflows/noirpanther_docker.yml) workflow on GitHub's native runners — pushing a `v<version>` tag builds the image and creates the GitHub release (Rust 1.97 / Node 24 toolchain; `CARGO_BUILD_JOBS` build-arg caps compile parallelism for small Docker VMs) — see [Install with Docker Compose](#install-with-docker-compose)

> A from-scratch **proprietary** client — **NoirPanther** — is built against this fork's API: <https://git.vint1024.net/vint1024/noirpanther.git>

The fork keeps the upstream **MIT** license.

---

<sub>The original upstream README follows. Its installation and Docker links describe upstream Stump — to install NoirPanther use [Install with Docker Compose](#install-with-docker-compose) above.</sub>

<p align="center">
  <img alt="Stump's logo. It depicts a young individual sitting on a tree stump reading a book. Inspired by the developer's childhood, where they spent a significant amount of time reading on a tree stump in their backyard" src="./.github/images/logo.png" style="width: 30%" />
  <br />
  <a href="https://github.com/awesome-selfhosted/awesome-selfhosted#document-management---e-books">
    <img src="https://cdn.rawgit.com/sindresorhus/awesome/d7305f38d29fed78fa85652e3a63e154dd8e8829/media/badge.svg" alt="Awesome Self-Hosted">
  </a>
  <a href="https://discord.gg/63Ybb7J3as">
    <img src="https://img.shields.io/discord/972593831172272148?label=Discord&color=5865F2" />
  </a>
  <a href="https://github.com/stumpapp/stump/blob/main/LICENSE">
    <img src="https://img.shields.io/static/v1?label=License&message=MIT&color=CF9977" />
  </a>
  <a href="https://hub.docker.com/r/aaronleopold/stump">
    <img src="https://img.shields.io/docker/pulls/aaronleopold/stump?logo=docker&color=0aa8d2&logoColor=fff" alt="Docker Pulls">
  </a>
</p>

<p align='center'>

Stump is a free and open source comics, manga, and digital book server with OPDS support, created with <a href="https://www.rust-lang.org/">Rust</a>, <a href='https://github.com/tokio-rs/axum'>Axum</a>, <a href='https://www.sea-ql.org/SeaORM/'>SeaORM</a> and <a href='https://reactjs.org/'>React</a>.

</p>

<p align='center'>
<img alt="Screenshot of Stump" src="./docs/public/images/landing-dark.png" style="width: 90%" />
</p>

<!-- prettier-ignore: I hate you sometimes prettier -->
<details>
  <summary><b>Table of Contents</b></summary>
  <p>

- [Disclaimer](#disclaimer)
- [Features](#features)
- [Roadmap](#roadmap)
- [Getting Started](#getting-started)
- [Developer Guide](#developer-guide)
  - [Contributing](#contributing)
- [Repository Structure](#repository-structure)
- [Similar Projects](#similar-projects)
- [License](#license)
- [Attribution](#attribution)
</details>

## Disclaimer

Stump is under active development and should be treated as **beta software** until it reaches a stable `1.0` release. I do my best to avoid breaking changes, or changes which might cause data loss, but there are no guarantees.

I develop and maintain Stump in my free time. In other words, this is not my job and there is no guarantee of any timeline for features or bug fixes.

## Features

- [OPDS](https://opds.io/) [v1.2](https://specs.opds.io/opds-1.2) (including [OPDS PSE](https://github.com/anansi-project/opds-pse)) and [v2.0](https://specs.opds.io/opds-2.0.html) support
- EPUB, PDF, CBZ/ZIP, and CBR/RAR support
- Built-in readers for all supported formats
- Annotations and highlights for EPUB books
- OIDC authentication
- Translations with [Weblate](https://weblate.org/en/)
- Multi-user account management with permissions, age restrictions, and other access control features
- Theming support with a handful of [built-in themes](https://www.stumpapp.dev/docs/apps/web/themes)
- [Kobo](https://www.stumpapp.dev/docs/guides/integrations/kobo) and [KoReader](https://www.stumpapp.dev/docs/guides/integrations/koreader) sync integrations
- Multiple different installation methods, including Docker and pre-built binaries

And more not mentioned. The [documentation](https://www.stumpapp.dev) will provide additional details about features, installation, and usage guides.

## Roadmap

You can track the [project boards](https://github.com/stumpapp/stump/projects?query=is%3Aopen) to see what efforts are currently being worked on or planned.

Feel free to create an issue or discussion if you have anything else you'd like to see!

## Getting Started

The installation guides are available in the [documentation](https://www.stumpapp.dev/docs/getting-started/installation) (or [the markdown](/docs/content/docs/getting-started/installation/index.mdx), if you prefer).

## Developer Guide

The developer guide is available in the [documentation](https://www.stumpapp.dev/docs/developer/contributing) (or [the markdown](/docs/content/docs/developer/contributing.mdx), if you prefer). To not have to maintain two copies of the same information, please refer to those links for the most up-to-date information.

## Contributing

Contributions are very **welcome**! Please review the [CONTRIBUTING.md](./.github/CONTRIBUTING.md) before getting started.

I recommend taking a look at [open issues](https://github.com/stumpapp/stump/issues). You can also check out the [project boards](https://github.com/stumpapp/stump/projects?query=is%3Aopen) to see what efforts are active or planned.

In general, the following areas could always use help:

- Translations via [Weblate](https://hosted.weblate.org/engage/stump/), so Stump is accessible to as many people as possible
- Writing comprehensive tests
- Improving the UI/UX, even small changes can go a long way
- CI pipelines, automated release processes, and other devops-related efforts
- Addressing `TODO` or `FIXME` comments in the codebase

### Repository Structure

The repository is managed via yarn workspaces and cargo workspaces:

```bash
# The primary applications all grouped together
apps/
  desktop/   # Tauri wrapping the web UI
  expo/      # React Native app
  server/    # Axum server
  web/       # UI served by the server
# The primary internals, like file processing etc
core/
# Supporting Rust crates (cli, graphql, integrations, etc)
crates/
  migrations/  # Database migrations
  models/      # Database models
docs/
# Shared TypeScript packages
packages/
```

## Translations

[![Translation status](https://hosted.weblate.org/widgets/stump/-/stump/horizontal-auto.svg)](https://hosted.weblate.org/engage/stump/)

## Similar Projects

There are a number of other projects that are similar to Stump, it certainly isn't the first or only digital book media server out there. If Stump isn't for you, or you want to check out similar projects in this space, here are some other projects you might be interested in:

- [audiobookshelf](https://github.com/advplyr/audiobookshelf) (_Audiobooks, Podcasts_)
- [Codex](https://github.com/ajslater/codex)
- [Kavita](https://github.com/Kareadita/Kavita)
- [Komga](https://github.com/gotson/komga)
- [Storyteller](https://gitlab.com/storyteller-platform/storyteller)

## License

> If a package or subfolder has its own license file, that license takes precedence over the repository-level license and will be listed below.

- The [expo application](./apps/expo/LICENSE) is licensed under [GPL-3.0](./apps/expo/LICENSE) ([summary](https://www.gnu.org/licenses/gpl-3.0.html))
- The [`wifi-ssid` native module](./apps/expo/modules/wifi-ssid) was sourced from Streamyfin and licensed under [MPL-2.0](https://www.mozilla.org/en-US/MPL/2.0/) ([summary](https://www.tldrlegal.com/license/mozilla-public-license-2-0-mpl-2))
- All other code in the repository is licensed under [MIT License](./LICENSE) ([summary](https://www.tldrlegal.com/license/mit-license))

## Attribution

- Some of the icons used in the web and mobile applications are from the [Spacedrive](https://github.com/spacedriveapp/spacedrive/tree/main/packages/assets/icons) repository, and are licensed under the [FSL-1.1-ALv2](https://github.com/spacedriveapp/spacedrive/blob/main/LICENSE) license.
- The native Readium expo modules were adapted from [Storyteller](https://gitlab.com/storyteller-platform/storyteller)
