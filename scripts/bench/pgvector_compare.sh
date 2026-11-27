#!/bin/sh
# Runs the comparison of VI15 (vector indexes PRD 7.6) with PostgreSQL and pgvector on this machine: PostgreSQL in the
# `pgvector/pgvector:pg17` container, the client (`pgvector_compare.py`) in a `python:3.13-slim` container, both on the
# host's network, PostgreSQL listening on 127.0.0.1 only, the data of `krummi-ann-lab preparar` mounted read-only; the
# container and its throwaway password go when the run ends.
#
# Usage: scripts/bench/pgvector_compare.sh <data folder> <report> [arguments of pgvector_compare.py]
set -eu
data=$(cd "$1" && pwd); report=$2; shift 2
here=$(cd "$(dirname "$0")" && pwd)
out=$(cd "$(dirname "$report")" && pwd)/$(basename "$report")
docker rm -f krummi-pgvector >/dev/null 2>&1 || true
docker run -d --name krummi-pgvector --network host -e POSTGRES_PASSWORD=bench \
  pgvector/pgvector:pg17 -c listen_addresses=127.0.0.1 -c port=55432 -c shared_buffers=8GB -c max_wal_size=8GB >/dev/null
until docker exec krummi-pgvector pg_isready -p 55432 -q; do sleep 1; done
sleep 2
docker run --rm --network host -v "$data:/data:ro" -v "$here:/scripts:ro" -v "$(dirname "$out"):/out" \
  python:3.13-slim sh -c "pip install -q 'psycopg[binary]' && python /scripts/pgvector_compare.py /data /out/$(basename "$out") $*"
docker rm -f krummi-pgvector >/dev/null
