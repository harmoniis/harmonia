#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REF_DIR="${ROOT_DIR}/doc/reference"
MIGRATION_MAP="${REF_DIR}/migration-map.md"
SECTION_MATRIX="${REF_DIR}/source-section-coverage.md"
GEN_SCRIPT="${SCRIPT_DIR}/generate-doc-section-coverage.sh"

errors=()

if [[ ! -f "${MIGRATION_MAP}" ]]; then
  errors+=("missing migration map: ${MIGRATION_MAP}")
fi

if [[ ! -x "${GEN_SCRIPT}" ]]; then
  errors+=("missing or non-executable generator: ${GEN_SCRIPT}")
fi

CANON_MODE=""
if [[ -d "${ROOT_DIR}/../doc/agent/genesis" && -d "${ROOT_DIR}/../doc/agent/evolution/latest" ]]; then
  CANON_MODE="doc-agent"
  CANON_GENESIS="${ROOT_DIR}/../doc/agent/genesis"
  CANON_EVOLUTION="${ROOT_DIR}/../doc/agent/evolution/latest"
  SOURCE_PREFIX="../../../doc/agent"
elif [[ -d "${ROOT_DIR}/src/boot/genesis" && -d "${ROOT_DIR}/src/boot/evolution/latest" ]]; then
  CANON_MODE="src-boot"
  CANON_GENESIS="${ROOT_DIR}/src/boot/genesis"
  CANON_EVOLUTION="${ROOT_DIR}/src/boot/evolution/latest"
  SOURCE_PREFIX="../../src/boot"
else
  errors+=("canonical docs missing under doc/agent and src/boot")
fi

if (( ${#errors[@]} > 0 )); then
  printf '[doc-coverage] %s\n' "${errors[@]}" >&2
  exit 1
fi

doc_source_ref() {
  local doc="$1"
  if [[ "${doc}" == "${CANON_GENESIS}"/* ]]; then
    printf '%s/genesis/%s' "${SOURCE_PREFIX}" "${doc#${CANON_GENESIS}/}"
  else
    printf '%s/evolution/latest/%s' "${SOURCE_PREFIX}" "${doc#${CANON_EVOLUTION}/}"
  fi
}

DOCS=()
while IFS= read -r doc; do
  DOCS+=("${doc}")
done < <(
  {
    find "${CANON_GENESIS}" -maxdepth 1 -type f -name '*.md' -print
    find "${CANON_EVOLUTION}" -maxdepth 1 -type f -name '*.md' -print
  } | sort
)

for doc in "${DOCS[@]}"; do
  source_ref="$(doc_source_ref "${doc}")"
  map_pattern="$(printf '| `%s` |' "${source_ref}")"
  if ! grep -Fq "${map_pattern}" "${MIGRATION_MAP}"; then
    errors+=("migration-map missing source entry: ${source_ref}")
  fi
done

mapped_refs=()
while IFS= read -r ref; do
  mapped_refs+=("${ref}")
done < <(grep -oE '(doc/agent|src/boot)/(genesis|evolution/latest)/[^` ]+\.md' "${MIGRATION_MAP}" | sort -u || true)

for ref in "${mapped_refs[@]}"; do
  if [[ "${ref}" == doc/agent/* ]]; then
    abs="${ROOT_DIR}/../${ref}"
  else
    abs="${ROOT_DIR}/${ref}"
  fi
  if [[ ! -f "${abs}" ]]; then
    errors+=("migration-map stale source path: ${ref}")
  fi
done

tmp_file="$(mktemp)"
cleanup() {
  rm -f "${tmp_file}"
}
trap cleanup EXIT

bash "${GEN_SCRIPT}" "${tmp_file}" >/dev/null

if [[ ! -f "${SECTION_MATRIX}" ]]; then
  errors+=("missing section coverage file: ${SECTION_MATRIX}")
else
  if ! diff -u "${SECTION_MATRIX}" "${tmp_file}" > /tmp/doc-coverage.diff.$$; then
    errors+=("section coverage file out of date: ${SECTION_MATRIX} (run scripts/generate-doc-section-coverage.sh)")
    cat /tmp/doc-coverage.diff.$$ >&2 || true
  fi
  rm -f /tmp/doc-coverage.diff.$$ || true
fi

if (( ${#errors[@]} > 0 )); then
  printf '[doc-coverage] %s\n' "${errors[@]}" >&2
  exit 1
fi

echo "[doc-coverage] OK: migration-map and section coverage are complete and up to date."
