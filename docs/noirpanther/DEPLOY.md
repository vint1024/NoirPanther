# Deploying a server release

Four servers run this fork. **The order is fixed and not negotiable:**

```
local :10912  →  smoke it  →  home :10802  →  home :10803  →  public demo
```

The local stand exists so a bad release is caught before it reaches ~14 000 books of someone's
library and a demo that app-store reviewers open. Deploying to production first has cost us a
round trip before; don't repeat it.

Prerequisites: the tag's `NoirPanther Docker image` workflow finished **successfully**
(`gh run list --repo vint1024/NoirPanther --limit 5`), and `:latest` points at the new release:

```bash
docker manifest inspect ghcr.io/vint1024/noirpanther:latest | grep -m1 digest
docker manifest inspect ghcr.io/vint1024/noirpanther:0.1.9-rN | grep -m1 digest   # same digest
```

A run can fail on one architecture and be re-run: `gh run rerun <id> --failed --repo vint1024/NoirPanther`.
GitHub mails you about the failure and stays silent about the successful re-run.

---

## 1. Local `:10912`

The stack lives in `~/projects/stump/.local/noirpanther-local`. **The compose service is `stump`**
(the _container_ is `noir-cattest` — that name does not work with `docker compose`).

```bash
cd ~/projects/stump/.local/noirpanther-local
cp docker-compose.yml docker-compose.yml.bak-$(date +%Y%m%d_%H%M)
sed -i '' 's|image: .*noirpanther:.*|image: ghcr.io/vint1024/noirpanther:0.1.9-rN|' docker-compose.yml
sed -i '' 's|image: stump:vint-dev|image: ghcr.io/vint1024/noirpanther:0.1.9-rN|' docker-compose.yml
docker compose pull stump && docker compose up -d stump
sleep 12 && curl -s -m 10 -X POST http://localhost:10912/api/v2/version
```

To put the stand back on the working tree afterwards: restore the backup compose (image
`stump:vint-dev`) and run `bash ~/projects/stump/stump_original/.build-logs/deploy-local.sh`.

Then run the smoke of section 4 against `http://localhost:10912` with `cat` / `cattest12345`.
**Do not move on until it is green.**

## 2. Home servers `:10802` (live) and `:10803` (archive)

Read-only host except these two stacks ([[homeserver-readonly-rule]]). One at a time, live first.

```bash
SSH="ssh -o IdentitiesOnly=yes -o IdentityAgent=none -i $HOME/.ssh/claude_key homeserver@192.168.50.161"
# NOTE: run ssh spelled out, not through a shell variable — zsh splits it wrong
```

For `D` in `noir-panter` (:10802) and `noir-panter-lgbtlibrary` (:10803), with `SVC` = the same name:

```bash
# 1. back the database up first (the script keeps the 5 newest dumps)
ssh -o IdentitiesOnly=yes -o IdentityAgent=none -i ~/.ssh/claude_key homeserver@192.168.50.161 \
  "export PATH=/usr/local/bin:\$PATH; cd /Users/homeserver/docker/$D && ./backup-db.sh"

# 2. point the stack at the new tag, pull, recreate
ssh -o IdentitiesOnly=yes -o IdentityAgent=none -i ~/.ssh/claude_key homeserver@192.168.50.161 \
  "export PATH=/usr/local/bin:\$PATH; cd /Users/homeserver/docker/$D \
   && cp docker-compose.yml docker-compose.yml.bak-\$(date +%Y%m%d_%H%M) \
   && sed -i '' 's|noirpanther:[0-9].*|noirpanther:0.1.9-rN|' docker-compose.yml \
   && docker compose pull $SVC && docker compose up -d $SVC \
   && sleep 12 && docker compose ps --format '{{.Name}} {{.Status}} {{.Image}}'"
```

Then the checks of section 4 against `http://192.168.50.161:10802` (user `claude-test` /
`ClaudeTest-2026!`), and the book count against what it was **before** the deploy.

Rollback: the compose backup next to the file carries the previous tag — restore it, `up -d`.

## 3. Public demo

Only through Komodo — `docker compose` on that host is off limits ([[demo-server-update-duty]],
`~/projects/stump/DEMO_SERVER.md`).

```bash
set -a; . ~/projects/stump/.local/komodo/komodo.local; set +a
K() { curl -s -m 60 -X POST "$KOMODO_URL/$1/$2" -H 'Content-Type: application/json' \
      -H "X-Api-Key: $KOMODO_API_KEY" -H "X-Api-Secret: $KOMODO_API_SECRET" -d "$3"; }

K execute DeployStack '{"stack":"noirpanther-demo"}' | python3 -c \
  "import sys,json; d=json.loads(sys.stdin.read(), strict=False); print(d['_id'])"

# poll — NOTE: json.loads(..., strict=False); the logs carry raw control characters and a
# plain json.load() throws "Invalid control character"
K read GetUpdate '{"id":"<id>"}' | python3 -c "
import sys,json
d=json.loads(sys.stdin.read(), strict=False)
print(d['status'], d['success'])
for l in d['logs']: print(l['stage'], l['success'])
"
```

`status=Complete` **and** `success=true` — an HTTP 200 alone means nothing. Then section 4 against
`https://demo.noirpanther.kuvshinov.in` (`demo` / `demo1234`), plus:

```bash
ssh -i ~/.ssh/claude_key -o IdentitiesOnly=yes -o IdentityAgent=none -o BatchMode=yes \
  root@web-monitoring.vint1024.net \
  "docker ps --filter name=noirpanther-demo --format '{{.Names}} {{.Status}} {{.Image}}'; \
   docker exec noirpanther-demo-db psql -U stump -d stump -tAc \
   \"select tgname from pg_trigger where tgname='npdemo_freeze_demo'\""
```

The trigger must still be there — it is what keeps the demo account read-only. If it is gone,
restore it from `/opt/noirpanther-demo/demo-freeze.sql` and tell the owner.

## 4. The per-server smoke (run it on every one of them)

```bash
S=http://localhost:10912; U=cat; P=cattest12345      # adjust per server

curl -s -m 10 -X POST $S/api/v2/version                        # semver + buildChannel stable
curl -s -m 10 -o /dev/null -w '%{content_type}\n' $S/manifest.webmanifest
                                                               # application/manifest+json, not HTML
curl -s -m 10 -I $S/ | grep -iE 'content-security-policy|x-content-type-options|referrer-policy'

# NOTE: quote the URL — zsh eats the `?` otherwise ("no matches found")
T=$(curl -s -m 15 -X POST "$S/api/v2/auth/login?generate_token=true" \
    -H 'Content-Type: application/json' -d "{\"username\":\"$U\",\"password\":\"$P\"}" \
    | python3 -c "import sys,json;print(json.load(sys.stdin)['accessToken'])")

# NOTE: pageInfo is a union — totalItems only exists on the OffsetPaginationInfo branch
curl -s -m 25 -X POST $S/api/graphql -H "Authorization: Bearer $T" \
  -H 'Content-Type: application/json' \
  -d '{"query":"{ media(pagination:{offset:{page:1,pageSize:2}}) { pageInfo { ... on OffsetPaginationInfo { totalItems } } nodes { name } } libraries { nodes { name } } }"}'
```

Container logs must be free of migration errors (`FORCE_DB_RESET … NotPresent` is a harmless
warning on every start):

```bash
docker logs <container> --since 5m 2>&1 | grep -iE 'error|panic|migrat' | grep -v FORCE_DB
```

In a browser, on the local stand at least: log in, open an EPUB (two columns of text, no CSP
errors in the console), open a comic (pages load). Reader routes are
`/books/<id>/epub-reader` and `/books/<id>/reader?page=1` — follow the buttons rather than typing
paths; guessing them gives a 404.

> Service-worker registration fails on `http://localhost` inside Claude's browser pane
> ("An unknown error occurred when fetching the script"). It works over https (checked on the
> demo) — that is the pane, not a regression.

## 5. After the deploy

- Note the release and the four versions in memory's `current-state`.
- If the release changed metadata parsing, the libraries need a `forceRebuild` rescan; the demo
  needs the infra_management agent for that (we don't have its owner password).
