#!/usr/bin/env python3
"""
Copy a Stump (NoirPanther fork) SQLite database into a PostgreSQL database.

Usage:
  1. Start the server ONCE against the empty PostgreSQL database so the
     migrations create the schema (STUMP_DB_* / STUMP_DATABASE_URL env), then
     stop it (or keep it stopped during the copy).
  2. python3 sqlite_to_postgres.py --sqlite /config/stump.db \
       --pg postgresql://stump:pass@localhost:5432/stump
  3. Start the server again on PostgreSQL.

What it does: for every table present on both sides (except the migration
bookkeeping table), truncates the PostgreSQL table, converts each SQLite row to
the PostgreSQL column types (bool / timestamptz / json / bytea / date / numeric)
and inserts it with foreign-key checks deferred (session_replication_role =
replica), then resets the serial sequences and verifies row counts.

Requires: psycopg 3 (`pip install "psycopg[binary]"`). Idempotent — re-running
replaces the PostgreSQL data with the SQLite data again.
"""

import argparse
import datetime as dt
import re
import sqlite3
import sys

import psycopg

SKIP_TABLES = {"seaql_migrations", "sqlite_sequence"}
BATCH = 500


def parse_ts(value):
    """SQLite stores RFC3339 strings (sometimes with nanoseconds) or epoch-ish ints."""
    if value is None:
        return None
    if isinstance(value, (int, float)):
        return dt.datetime.fromtimestamp(value, tz=dt.timezone.utc)
    s = str(value).strip()
    if not s:
        return None
    # Truncate fractional seconds to microseconds (Postgres max) before parsing.
    s = re.sub(r"(\.\d{6})\d+", r"\1", s)
    s = s.replace("Z", "+00:00")
    if " " in s and "T" not in s:
        s = s.replace(" ", "T", 1)
    try:
        d = dt.datetime.fromisoformat(s)
    except ValueError:
        # e.g. "2026-06-12 23:43:26" without a zone
        d = dt.datetime.strptime(s[:19], "%Y-%m-%dT%H:%M:%S")
    return d


def convert(value, data_type):
    if value is None:
        return None
    if data_type == "boolean":
        if isinstance(value, str):
            return value.strip().lower() in ("1", "true", "t", "yes")
        return bool(value)
    if data_type == "timestamp with time zone":
        d = parse_ts(value)
        if d is not None and d.tzinfo is None:
            d = d.replace(tzinfo=dt.timezone.utc)
        return d
    if data_type == "timestamp without time zone":
        d = parse_ts(value)
        if d is not None and d.tzinfo is not None:
            d = d.astimezone(dt.timezone.utc).replace(tzinfo=None)
        return d
    if data_type == "date":
        s = str(value)[:10]
        return dt.date.fromisoformat(s)
    if data_type in ("json", "jsonb"):
        if isinstance(value, bytes):
            value = value.decode("utf-8")
        return str(value)
    if data_type == "bytea":
        if isinstance(value, str):
            return value.encode("utf-8")
        return bytes(value)
    if data_type in ("integer", "bigint", "smallint"):
        if isinstance(value, str) and value.strip() == "":
            return None
        return int(value)
    if data_type in ("numeric", "real", "double precision"):
        if isinstance(value, str) and value.strip() == "":
            return None
        return float(value)
    # text / character varying
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return str(value) if not isinstance(value, str) else value


def placeholder(data_type):
    if data_type in ("json", "jsonb"):
        return f"%s::{data_type}"
    return "%s"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sqlite", required=True)
    ap.add_argument("--pg", required=True, help="postgresql://user:pass@host:port/db")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    src = sqlite3.connect(f"file:{args.sqlite}?mode=ro", uri=True)
    src.row_factory = sqlite3.Row
    dst = psycopg.connect(args.pg, autocommit=False)

    sqlite_tables = [
        r[0]
        for r in src.execute(
            "select name from sqlite_master where type='table' and name not like 'sqlite_%' order by name"
        )
    ]
    with dst.cursor() as cur:
        cur.execute(
            "select table_name, column_name, data_type from information_schema.columns "
            "where table_schema='public' order by table_name, ordinal_position"
        )
        pg_cols = {}
        for t, c, d in cur.fetchall():
            pg_cols.setdefault(t, []).append((c, d))
        cur.execute(
            "select table_name, column_name from information_schema.columns "
            "where table_schema='public' and (column_default like 'nextval%%' or is_identity='YES')"
        )
        serials = cur.fetchall()

    tables = [t for t in sqlite_tables if t not in SKIP_TABLES and t in pg_cols]
    missing = [t for t in sqlite_tables if t not in SKIP_TABLES and t not in pg_cols]
    if missing:
        print(f"WARNING: tables in SQLite but not in PostgreSQL (skipped): {missing}")

    report = []
    with dst.cursor() as cur:
        cur.execute("SET session_replication_role = replica")
        cur.execute("SET client_min_messages = warning")
        for t in tables:
            cur.execute(f'TRUNCATE TABLE "{t}" RESTART IDENTITY CASCADE')
        for t in tables:
            src_cols = [r[1] for r in src.execute(f'PRAGMA table_info("{t}")')]
            pg_types = dict(pg_cols[t])
            cols = [c for c in src_cols if c in pg_types]
            extra = [c for c in src_cols if c not in pg_types]
            if extra:
                print(f"  note: {t}: SQLite-only columns ignored: {extra}")
            if not cols:
                continue
            col_sql = ", ".join(f'"{c}"' for c in cols)
            ph = ", ".join(placeholder(pg_types[c]) for c in cols)
            insert = f'INSERT INTO "{t}" ({col_sql}) VALUES ({ph})'
            rows = src.execute(f'SELECT {col_sql} FROM "{t}"').fetchall()
            batch = []
            n = 0
            for row in rows:
                try:
                    batch.append(tuple(convert(row[c], pg_types[c]) for c in cols))
                except Exception as e:  # noqa: BLE001
                    raise SystemExit(f"{t}: cannot convert row {dict(row)}: {e}") from e
                if len(batch) >= BATCH:
                    cur.executemany(insert, batch)
                    n += len(batch)
                    batch = []
            if batch:
                cur.executemany(insert, batch)
                n += len(batch)
            report.append((t, len(rows), n))
        # Sequences: continue after the highest copied id.
        for t, c in serials:
            if t in tables:
                cur.execute(
                    f"SELECT setval(pg_get_serial_sequence('\"{t}\"', '{c}'), "
                    f'COALESCE((SELECT MAX("{c}") FROM "{t}"), 0) + 1, false)'
                )
        cur.execute("SET session_replication_role = DEFAULT")
        # Verify
        bad = []
        for t, expected, _ in report:
            cur.execute(f'SELECT COUNT(*) FROM "{t}"')
            got = cur.fetchone()[0]
            if got != expected:
                bad.append((t, expected, got))
    if args.dry_run:
        dst.rollback()
        print("dry run: rolled back")
    else:
        dst.commit()
    total = sum(n for _, _, n in report)
    print(f"copied {total} rows across {len(report)} tables")
    for t, expected, n in report:
        if n:
            print(f"  {t}: {n}")
    if bad:
        print(f"MISMATCH: {bad}")
        sys.exit(1)
    print("row counts verified")


if __name__ == "__main__":
    main()
