#!/bin/bash
# Dump the NoirPanther PostgreSQL database into ./db-backups (run from the stack directory, e.g. /Users/homeserver/docker/noir-panter).
set -euo pipefail
export PATH=/usr/local/bin:$PATH
cd "$(dirname "$0")"
mkdir -p db-backups
OUT="db-backups/stump-pg-$(date +%Y%m%d_%H%M).sql.gz"
docker compose exec -T db pg_dump -U stump -d stump --no-owner | gzip > "$OUT"
ls -la "$OUT"
