#!/usr/bin/env python3
"""Check Rust graph against an independently produced Elm 0.19.1 cache.

The cache must come from a clean compilation of this entry point; a cache
containing other entry points is not a valid oracle. Nothing is written there.
"""
import argparse
import json
import pathlib
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument("--manifest", type=pathlib.Path, required=True)
parser.add_argument("--entry", required=True)
parser.add_argument("--reference-cache", type=pathlib.Path, required=True)
args = parser.parse_args()
binary = pathlib.Path(__file__).resolve().parents[1] / "target/release/planexpo-elm"
result = subprocess.run(
    [str(binary), "graph", str(args.manifest), args.entry],
    check=True, text=True, capture_output=True,
)
graph = json.loads(result.stdout)
actual = {m["name"] for m in graph["modules"] if m["owner"] == "application"}
expected = {p.stem.replace("-", ".") for p in args.reference_cache.glob("*.elmi")}
if not expected:
    raise SystemExit("Reference cache has no .elmi files; compile with Elm first.")
if actual != expected:
    raise SystemExit(json.dumps({"only_rust": sorted(actual-expected), "only_elm": sorted(expected-actual)}))
print(json.dumps({"stage": "dependency-graph-parity", "local_modules": len(actual),
                  "total_nodes": len(graph["modules"]), "exact_match": True,
                  "rust_discovery_ms": graph["elapsed_ms"]}))
