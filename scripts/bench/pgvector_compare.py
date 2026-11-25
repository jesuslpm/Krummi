"""The informative comparison of VI15 of the vector indexes PRD (7.6) with PostgreSQL and pgvector: the same Cohere
data the engine measures (`krummi-ann-lab preparar`), one thread (`max_parallel_workers_per_gather = 0`), an IVFFlat
index of 1,000 lists with iterative searches (`ivfflat.iterative_scan`), and the measures the closing report compares:
the p95 of a search at each number of probes with its recall@10, without a filter and with a filter that lets few rows
pass and one that lets many pass; the p95 of inserting one row with and without the index; the p95 of an empty query
(`SELECT 1`), which is what the protocol costs; and the versions.

Run it through `pgvector_compare.sh`, which starts PostgreSQL and this client in containers on the same machine. Its
messages are in Spanish (repository rule).

Usage: python pgvector_compare.py <data folder> <report> [--filas N] [--sondeos 20,30,...]
"""

import argparse
import array
import struct
import sys
import time

import psycopg

QUERIES = 1000
FILTERED_QUERIES = 200
INSERTS = 2000
CATEGORIES = 100


def records(path, limit=None):
    """The vectors of an `fvecs` file, each as an `array('f')`, up to `limit`."""
    with open(path, "rb") as f:
        count = 0
        while limit is None or count < limit:
            head = f.read(4)
            if not head:
                return
            (dim,) = struct.unpack("<i", head)
            values = array.array("f")
            values.frombytes(f.read(4 * dim))
            yield values
            count += 1


def literal(values):
    return "[" + ",".join(repr(x) for x in values) + "]"


def percentile(times, p):
    times = sorted(times)
    return times[round((len(times) - 1) * p / 100)] * 1000.0


def read_truth(path):
    """The ids of each query's neighbours in an `ivecs` file, the first 10."""
    out = []
    with open(path, "rb") as f:
        while True:
            head = f.read(4)
            if not head:
                return out
            (dim,) = struct.unpack("<i", head)
            ids = array.array("i")
            ids.frombytes(f.read(4 * dim))
            out.append(list(ids[:10]))


def copy_rows(conn, base, rows):
    """Loads the base with a binary COPY: pgvector's binary form is the dimension, a reserved int16 and big-endian
    `float4`s."""
    with conn.cursor() as cur:
        with cur.copy("COPY docs (id, cat, v) FROM STDIN (FORMAT BINARY)") as copy:
            copy.write(b"PGCOPY\n\xff\r\n\x00" + struct.pack("!ii", 0, 0))
            chunk = []
            for i, values in enumerate(records(base, rows)):
                values.byteswap()
                vector = struct.pack("!hh", len(values), 0) + values.tobytes()
                chunk.append(struct.pack("!hiqiqi", 3, 8, i, 8, i % CATEGORIES, len(vector)) + vector)
                if len(chunk) == 10_000:
                    copy.write(b"".join(chunk))
                    chunk = []
            copy.write(b"".join(chunk) + struct.pack("!h", -1))
    conn.commit()


def timed_inserts(conn, queries, first):
    """The p50 and p95 of inserting one row per transaction (autocommit, synchronous)."""
    times = []
    with conn.cursor() as cur:
        for i in range(INSERTS):
            v = literal(queries[i % len(queries)])
            start = time.perf_counter()
            cur.execute(
                "INSERT INTO docs (id, cat, v) VALUES (%s, %s, %s::vector)",
                (first + i, (first + i) % CATEGORIES, v),
            )
            times.append(time.perf_counter() - start)
        cur.execute("DELETE FROM docs WHERE id >= %s", (first,))
        conn.commit()
    return percentile(times, 50), percentile(times, 95)


def search(conn, queries, count, where, probes, expected):
    """recall@10, p50 and p95 of the index search after one warm-up pass."""
    sql = f"SELECT id FROM docs {where} ORDER BY v <-> %s::vector LIMIT 10"
    with conn.cursor() as cur:
        cur.execute(f"SET ivfflat.probes = {probes}")
        texts = [literal(q) for q in queries[:count]]
        for t in texts:
            cur.execute(sql, (t,))
            cur.fetchall()
        times, hits = [], 0
        for t, truth in zip(texts, expected):
            start = time.perf_counter()
            cur.execute(sql, (t,))
            found = [r[0] for r in cur.fetchall()]
            times.append(time.perf_counter() - start)
            hits += len(set(found) & set(truth))
    return hits / (10 * count), percentile(times, 50), percentile(times, 95)


def exact_truth(conn, queries, count, where):
    """The exact 10 nearest with the filter, by a sequential scan."""
    sql = f"SELECT id FROM docs {where} ORDER BY v <-> %s::vector LIMIT 10"
    out = []
    with conn.cursor() as cur:
        cur.execute("SET enable_indexscan = off")
        cur.execute("SET enable_bitmapscan = off")
        cur.execute("SET max_parallel_workers_per_gather = 8")
        for q in queries[:count]:
            cur.execute(sql, (literal(q),))
            out.append([r[0] for r in cur.fetchall()])
        cur.execute("RESET enable_indexscan")
        cur.execute("RESET enable_bitmapscan")
        cur.execute("SET max_parallel_workers_per_gather = 0")
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("data")
    parser.add_argument("report")
    parser.add_argument("--filas", type=int, default=None)
    parser.add_argument("--sondeos", default="20,30,40,50,60,80")
    parser.add_argument("--dsn", default="host=127.0.0.1 port=55432 user=postgres password=bench dbname=postgres")
    args = parser.parse_args()
    probes = [int(p) for p in args.sondeos.split(",")]
    base, query_file = f"{args.data}/cohere-base.fvecs", f"{args.data}/cohere-query.fvecs"
    queries = list(records(query_file))
    rows = args.filas or 1_000_000
    expected = read_truth(f"{args.data}/cohere-l2-{rows}-exact.ivecs")
    lines = []

    def say(text):
        print(text, flush=True)
        lines.append(text)

    with psycopg.connect(args.dsn, autocommit=True) as conn:
        with conn.cursor() as cur:
            cur.execute("CREATE EXTENSION IF NOT EXISTS vector")
            cur.execute("DROP TABLE IF EXISTS docs")
            cur.execute("CREATE TABLE docs (id bigint PRIMARY KEY, cat bigint NOT NULL, v vector(768))")
            cur.execute("SELECT version(), (SELECT extversion FROM pg_extension WHERE extname = 'vector')")
            version, extension = cur.fetchone()
        conn.commit()
        start = time.perf_counter()
        copy_rows(conn, base, rows)
        load = time.perf_counter() - start
        with conn.cursor() as cur:
            cur.execute("SET max_parallel_workers_per_gather = 0")
            cur.execute("VACUUM ANALYZE docs")
        without = timed_inserts(conn, queries, 10_000_000)
        with conn.cursor() as cur:
            cur.execute("SET maintenance_work_mem = '8GB'")
            start = time.perf_counter()
            cur.execute("CREATE INDEX near ON docs USING ivfflat (v vector_l2_ops) WITH (lists = 1000)")
            conn.commit()
            build = time.perf_counter() - start
        with_index = timed_inserts(conn, queries, 10_000_000)
        with conn.cursor() as cur:
            cur.execute("VACUUM ANALYZE docs")
            cur.execute("SET max_parallel_workers_per_gather = 0")
            cur.execute("SET ivfflat.iterative_scan = relaxed_order")
            times = []
            for _ in range(1000):
                start = time.perf_counter()
                cur.execute("SELECT 1")
                cur.fetchone()
                times.append(time.perf_counter() - start)
        say(f"## PostgreSQL con pgvector ({rows} filas de 768 dimensiones)\n")
        say(f"- PostgreSQL: {version.split(',')[0]}; pgvector {extension}.")
        say("- Un hilo (`max_parallel_workers_per_gather = 0`), `ivfflat.iterative_scan = relaxed_order`, IVFFlat de 1000 listas.\n")
        say("| Medida | Valor |\n|---|---|")
        say(f"| Carga (COPY binario) | {load:.1f} s |")
        say(f"| Crear el índice IVFFlat | {build:.1f} s |")
        say(f"| Insertar una fila sin el índice (p50 / p95) | {without[0]:.3f} / {without[1]:.3f} ms |")
        say(f"| Insertar una fila con el índice (p50 / p95) | {with_index[0]:.3f} / {with_index[1]:.3f} ms |")
        say(f"| Razón del p95, con / sin | {with_index[1] / without[1]:.3f} |")
        say(f"| `SELECT 1` (p50 / p95) | {percentile(times, 50):.3f} / {percentile(times, 95):.3f} ms |\n")
        say(f"### Sin filtro ({QUERIES} consultas, k = 10)\n")
        say("| Sondeos | recall@10 | p50 ms | p95 ms |\n|---|---|---|---|")
        for p in probes:
            recall, p50, p95 = search(conn, queries, QUERIES, "", p, expected)
            say(f"| {p} | {recall:.4f} | {p50:.2f} | {p95:.2f} |")
        say(f"\n### Con filtro ({FILTERED_QUERIES} consultas, k = 10)\n")
        say("| Filtro | Sondeos | recall@10 | p50 ms | p95 ms |\n|---|---|---|---|---|")
        for label, where in [("cat = 7", "WHERE cat = 7"), ("cat < 50", "WHERE cat < 50")]:
            truth = exact_truth(conn, queries, FILTERED_QUERIES, where)
            for p in probes:
                recall, p50, p95 = search(conn, queries, FILTERED_QUERIES, where, p, truth)
                say(f"| {label} | {p} | {recall:.4f} | {p50:.2f} | {p95:.2f} |")
    with open(args.report, "a", encoding="utf-8") as out:
        out.write("\n".join(lines) + "\n\n")


if __name__ == "__main__":
    sys.exit(main())
