#!/usr/bin/env bash
# Disposable local Postgres for developing/testing gridtui, without touching
# any real Postgres you might already have running. Uses plain `initdb` +
# `pg_ctl` (no Docker/Podman needed for Postgres — MySQL is the one that
# needs a container, see dev-mysql.sh) in a throwaway data directory.
#
# Usage:
#   scripts/dev-postgres.sh up      # init (if needed) + start on :5599
#   scripts/dev-postgres.sh down    # stop + wipe the data directory
#   scripts/dev-postgres.sh psql    # open a psql shell against it
#   scripts/dev-postgres.sh dsn     # print the DSN gridtui / psql can use
set -euo pipefail

PORT=5599
DATA_DIR="${TMPDIR:-/tmp}/gridtui-dev-postgres"
LOG_FILE="$DATA_DIR.log"
PG_USER="gridtui"
PG_DB="gridtui_dev"

dsn() {
  echo "host=127.0.0.1 port=$PORT user=$PG_USER dbname=$PG_DB"
}

up() {
  if pg_ctl -D "$DATA_DIR" status >/dev/null 2>&1; then
    echo "already running (port $PORT)"
    return 0
  fi
  if [ ! -d "$DATA_DIR" ]; then
    initdb -D "$DATA_DIR" -U "$PG_USER" --auth=trust
  fi
  # unix_socket_directories='' avoids "socket path too long" failures when
  # $TMPDIR is a deeply-nested sandbox path (common in CI/agent sandboxes).
  pg_ctl -D "$DATA_DIR" -o "-p $PORT -k ''" -l "$LOG_FILE" start
  # First boot only: create the dev database (initdb's default DB is the
  # username, not $PG_DB).
  createdb -h 127.0.0.1 -p "$PORT" -U "$PG_USER" "$PG_DB" 2>/dev/null || true
  echo "up on port $PORT -- DSN: $(dsn)"
}

down() {
  pg_ctl -D "$DATA_DIR" stop -m fast 2>/dev/null || true
  rm -rf "$DATA_DIR" "$LOG_FILE"
  echo "stopped and wiped $DATA_DIR"
}

case "${1:-}" in
  up) up ;;
  down) down ;;
  psql) psql -h 127.0.0.1 -p "$PORT" -U "$PG_USER" -d "$PG_DB" ;;
  dsn) dsn ;;
  *) echo "usage: $0 {up|down|psql|dsn}" >&2; exit 1 ;;
esac
