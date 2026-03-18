#!/usr/bin/env bash
# Publish all Harmonia crates to crates.io in workspace dependency order.
# Usage: ./scripts/publish-all.sh [--dry-run]
set -euo pipefail

DRY_RUN=""
if [[ "${1:-}" == "--dry-run" ]]; then
  DRY_RUN="--dry-run"
  echo "==> Dry-run mode"
fi

SLEEP=75

publish() {
  local crate="$1"
  echo "--- Publishing ${crate} ---"
  local output
  if output=$(cargo publish -p "${crate}" ${DRY_RUN} 2>&1); then
    echo "${output}"
  else
    if echo "${output}" | grep -q "already exists"; then
      echo "    ${crate} already published, skipping"
    else
      echo "${output}" >&2
      echo "    ERROR: failed to publish ${crate}"
      return 1
    fi
  fi
  if [[ -z "${DRY_RUN}" ]]; then
    echo "    Waiting ${SLEEP}s for crates.io index..."
    sleep "${SLEEP}"
  fi
}

publish_order() {
  local metadata
  metadata="$(cargo metadata --quiet --format-version 1)"
  printf '%s' "${metadata}" | python3 - <<'PY'
import json
import sys
from collections import defaultdict
from heapq import heapify, heappop, heappush

meta = json.load(sys.stdin)
workspace = set(meta["workspace_members"])
packages = {pkg["id"]: pkg for pkg in meta["packages"] if pkg["id"] in workspace}
nodes = {node["id"]: node for node in meta["resolve"]["nodes"] if node["id"] in workspace}

forward = {pkg_id: set() for pkg_id in packages}
indegree = {pkg_id: 0 for pkg_id in packages}

for pkg_id, node in nodes.items():
    for dep in node["deps"]:
        dep_id = dep["pkg"]
        if dep_id not in packages:
            continue
        if pkg_id in forward[dep_id]:
            continue
        forward[dep_id].add(pkg_id)
        indegree[pkg_id] += 1

ready = sorted(pkg["name"] for pkg_id, pkg in packages.items() if indegree[pkg_id] == 0)
heapify(ready)
id_by_name = {pkg["name"]: pkg_id for pkg_id, pkg in packages.items()}
order = []

while ready:
    name = heappop(ready)
    pkg_id = id_by_name[name]
    order.append(name)
    for dependent in sorted(forward[pkg_id], key=lambda dep_id: packages[dep_id]["name"]):
        indegree[dependent] -= 1
        if indegree[dependent] == 0:
            heappush(ready, packages[dependent]["name"])

if len(order) != len(packages):
    missing = sorted(pkg["name"] for pkg_id, pkg in packages.items() if pkg["name"] not in order)
    raise SystemExit(
        f"failed to resolve publish order for workspace packages: {', '.join(missing)}"
    )

for name in order:
    print(name)
PY
}

echo "=== Publishing workspace crates in dependency order ==="
while IFS= read -r crate; do
  [[ -n "${crate}" ]] || continue
  publish "${crate}"
done < <(publish_order)

echo "=== Done ==="
