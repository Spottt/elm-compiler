#!/usr/bin/env python3
"""Parse every reachable Elm module plus optional extra source trees.

Success proves source parsing on this corpus, not name resolution, type checking
or equivalent JavaScript. Kernel JavaScript is deliberately excluded.
"""
import argparse
import json
import pathlib
import subprocess

p = argparse.ArgumentParser()
p.add_argument('--manifest', type=pathlib.Path, required=True)
p.add_argument('--entry', required=True)
p.add_argument('--source-tree', type=pathlib.Path, action='append', default=[])
a = p.parse_args()
binary = pathlib.Path(__file__).resolve().parents[1] / 'target/release/planexpo-elm'
graph = json.loads(subprocess.run([str(binary),'graph',str(a.manifest),a.entry],check=True,capture_output=True,text=True).stdout)
paths = {pathlib.Path(m['path']).resolve() for m in graph['modules'] if not m['kernel']}
for tree in a.source_tree:
    paths.update(f.resolve() for f in tree.rglob('*.elm'))
result = subprocess.run([str(binary),'parse',*[str(f) for f in sorted(paths)]],capture_output=True,text=True)
if result.returncode:
    raise SystemExit(result.stderr)
print(result.stdout, end='')
