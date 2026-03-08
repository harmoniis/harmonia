#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_DIR="$(cd "${ROOT_DIR}/.." && pwd)"
ALLOWLIST_FILE="${ROOT_DIR}/config/structure-allowlist.txt"

limit_for_ext() {
  case "$1" in
    rs) echo 320 ;;
    lisp) echo 320 ;;
    *) echo "" ;;
  esac
}

is_ignored_path() {
  local p="$1"
  [[ "$p" == *"/target/"* ]] && return 0
  [[ "$p" == *"/.cxx/"* ]] && return 0
  [[ "$p" == *"/build/"* ]] && return 0
  [[ "$p" == *"/DerivedData/"* ]] && return 0
  return 1
}

is_allowlisted() {
  local p="$1"
  [[ -f "$ALLOWLIST_FILE" ]] || return 1
  grep -vE '^\s*#|^\s*$' "$ALLOWLIST_FILE" | grep -Fxq "$p"
}

violations=()
allowlisted_hits=()

while IFS= read -r -d '' file; do
  rel="${file#${REPO_DIR}/}"
  if is_ignored_path "$rel"; then
    continue
  fi

  ext="${file##*.}"
  limit="$(limit_for_ext "$ext")"
  [[ -n "${limit}" ]] || continue

  lines=$(wc -l < "$file" | tr -d '[:space:]')
  if (( lines > limit )); then
    if is_allowlisted "$rel"; then
      allowlisted_hits+=("$lines/$limit $rel")
    else
      violations+=("$lines/$limit $rel")
    fi
  fi
done < <(find "$REPO_DIR/harmonia" "$REPO_DIR/harmoniislib" -type f \( -name '*.rs' -o -name '*.lisp' \) -print0)

if ((${#allowlisted_hits[@]} > 0)); then
  echo "[structure] allowlisted large files (planned debt):"
  printf '  %s\n' "${allowlisted_hits[@]}"
fi

if ((${#violations[@]} > 0)); then
  echo "[structure] violations detected:"
  printf '  %s\n' "${violations[@]}"
  echo "[structure] add intentional temporary exceptions to: ${ALLOWLIST_FILE}"
  exit 1
fi

echo "[structure] OK: no non-allowlisted monolith files exceeded limits."
