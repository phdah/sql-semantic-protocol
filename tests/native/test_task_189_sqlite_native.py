"""TASK-189: independent, executable SQLite conditional window/session oracles.

SQLite evidence is not extrapolated to other engines. No third-party
packages or protocol code are used to compute the expected results.
"""

import json
import math
from pathlib import Path
import sqlite3
import unittest

FREEZE = Path(__file__).resolve().parents[2] / "docs/task-189-acceptance-freeze.json"


class ConditionalSQLiteOracles(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        matrix = json.loads(FREEZE.read_text())
        cls.cases = {
            case["id"]: case
            for case in matrix["cases"] + matrix["numeric_oracle_cases"]
            if case["proof"] == "Residual_expected"
        }
        assert len(cls.cases) == 8
        print(f"SQLite native engine: {sqlite3.sqlite_version}")

    def source(self, case):
        db = sqlite3.connect(":memory:")
        datatype = case.get("source_schema", {}).get("x", "INTEGER")
        db.execute(f"CREATE TABLE s(id INTEGER, k INTEGER, x {datatype})")
        db.executemany(
            "INSERT INTO s(id,k,x) VALUES (?,?,?)",
            ((r["id"], r["k"], r["x"]) for r in case["source_bag"]),
        )
        actual = db.execute("SELECT id,k,x FROM s ORDER BY id").fetchall()
        self.assertEqual(len(actual), len(case["source_bag"]))
        for row, expected in zip(actual, case["source_bag"]):
            self.assertEqual(row[:2], (expected["id"], expected["k"]))
            if expected["x"] is None:
                self.assertIsNone(row[2])
            else:
                self.assertEqual(float(row[2]), float(expected["x"]))
        return db

    def test_distinct_has_native_engine_boundary(self):
        case = self.cases["74-C189-C1"]
        with self.assertRaisesRegex(
            sqlite3.OperationalError, "DISTINCT is not supported for window functions"
        ):
            self.source(case).execute(case["sql"]).fetchall()

    def test_filter_and_implicit_frame_full_bags(self):
        for key in ("74-C189-C2", "74-C189-C3"):
            with self.subTest(case=key):
                case = self.cases[key]
                output = self.source(case).execute(
                    f"SELECT * FROM ({case['sql']}) ORDER BY id"
                ).fetchall()
                self.assertEqual(
                    [r[0] for r in output], [r["id"] for r in case["source_bag"]]
                )
                actual = [None if value is None else str(value) for _, value in output]
                self.assertEqual(actual, case["native_duckdb_terminal_values_in_id_order"])

    def test_peer_order_does_not_certify_peer_identities(self):
        case = self.cases["74-C189-C4"]
        output = dict(self.source(case).execute(
            f"SELECT * FROM ({case['sql']}) ORDER BY id"
        ).fetchall())
        self.assertEqual(set(output), {r["id"] for r in case["source_bag"]})
        for row_id, value in ((1, 10), (2, 30), (3, 40), (6, None)):
            self.assertEqual(output[row_id], value)
        self.assertCountEqual([output[4], output[5]], [5, 10])

    def test_fractional_avg_and_numeric_affinity(self):
        expected = {
            "74-C189-C5": [5 / 3] * 3 + [5., 5., None],
            "74-C189-C6": [40.9] * 3 + [11., 11., None],
            "74-C189-N2": [0.3, 0.3],
            "74-C189-N3": [1999.5, 1999.5],
        }
        for key, values in expected.items():
            with self.subTest(case=key):
                case = self.cases[key]
                name = "mean" if key.endswith("C5") else "total"
                sql = (
                    f"SELECT id, CAST({name} AS TEXT), typeof({name}) "
                    f"FROM ({case['sql']}) ORDER BY id"
                )
                rows = self.source(case).execute(sql).fetchall()
                self.assertEqual(
                    [r[0] for r in rows], [r["id"] for r in case["source_bag"]]
                )
                for (_, actual, typ), wanted in zip(rows, values):
                    if wanted is None:
                        self.assertIsNone(actual)
                        self.assertEqual(typ, "null")
                    else:
                        self.assertTrue(math.isclose(float(actual), wanted, rel_tol=1e-14))
                if key in ("74-C189-C5", "74-C189-N2"):
                    self.assertEqual(rows[0][2], "real")


if __name__ == "__main__":
    unittest.main()
