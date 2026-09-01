#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [[ $# -gt 0 ]]; then
  components=("$@")
else
  components=(
    "vexometer"
    "vext"
    "vext-email-gateway"
    "vexometer-satellites"
    "lazy-eliminator"
    "vexometer-efficacy"
    "satellite-template"
  )
fi

failed=0

for component in "${components[@]}"; do
  component_dir="$ROOT_DIR/$component"
  manifest="$component_dir/.trust/trust-manifest.sha256"

  if [[ ! -f "$manifest" ]]; then
    echo "missing trust manifest: $manifest" >&2
    failed=1
    continue
  fi

  if (cd "$component_dir" && sha256sum -c .trust/trust-manifest.sha256 >/dev/null); then
    echo "trust manifest verified: $component"
  else
    echo "trust manifest verification failed: $component" >&2
    failed=1
  fi
done

exit "$failed"
