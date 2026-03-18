#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REF_DIR="${ROOT_DIR}/doc/reference"
MIGRATION_MAP="${REF_DIR}/migration-map.md"
SECTION_MATRIX="${REF_DIR}/source-section-coverage.md"
GEN_SCRIPT="${SCRIPT_DIR}/generate-doc-section-coverage.sh"

HAS_ERRORS=0

report_error() {
  printf '[doc-coverage] %s\n' "$1" >&2
  HAS_ERRORS=1
}

if [[ ! -f "${MIGRATION_MAP}" ]]; then
  report_error "missing migration map: ${MIGRATION_MAP}"
fi

if [[ ! -x "${GEN_SCRIPT}" ]]; then
  report_error "missing or non-executable generator: ${GEN_SCRIPT}"
fi

if [[ -d "${ROOT_DIR}/doc/genesis" && -d "${ROOT_DIR}/doc/evolution" ]]; then
  CANON_GENESIS="${ROOT_DIR}/doc/genesis"
  CANON_EVOLUTION="${ROOT_DIR}/doc/evolution"
else
  report_error "canonical docs missing under doc/genesis and doc/evolution"
fi

if (( HAS_ERRORS > 0 )); then
  exit 1
fi

doc_source_ref() {
  local doc="$1"
  if [[ "${doc}" == "${CANON_GENESIS}"/* ]]; then
    printf '../genesis/%s' "${doc#${CANON_GENESIS}/}"
  else
    printf '../evolution/%s' "${doc#${CANON_EVOLUTION}/}"
  fi
}

DOC_LIST="$(mktemp)"
TMP_MATRIX="$(mktemp)"
TMP_DIFF="$(mktemp)"

cleanup() {
  rm -f "${DOC_LIST}" "${TMP_MATRIX}" "${TMP_DIFF}"
}
trap cleanup EXIT

{
  find "${CANON_GENESIS}" -maxdepth 1 -type f -name '*.md' -print
  find "${CANON_EVOLUTION}" -maxdepth 1 -type f -name '*.md' -print
} | sort > "${DOC_LIST}"

DOC_COUNT="$(wc -l < "${DOC_LIST}" | tr -d '[:space:]')"
if [[ "${DOC_COUNT}" == "0" ]]; then
  report_error "canonical doc roots are empty"
fi

while IFS= read -r doc; do
  source_ref="$(doc_source_ref "${doc}")"
  map_pattern="$(printf '| `%s` |' "${source_ref}")"
  if ! grep -Fq "${map_pattern}" "${MIGRATION_MAP}"; then
    report_error "migration-map missing source entry: ${source_ref}"
  fi
done < "${DOC_LIST}"

while IFS= read -r ref; do
  [[ -n "${ref}" ]] || continue
  abs="${REF_DIR}/${ref}"
  if [[ ! -f "${abs}" ]]; then
    report_error "migration-map stale source path: ${ref}"
  fi
done < <(grep -oE '\.\./(genesis|evolution)/[^` ]+\.md' "${MIGRATION_MAP}" | sort -u || true)

bash "${GEN_SCRIPT}" "${TMP_MATRIX}" >/dev/null

if [[ ! -f "${SECTION_MATRIX}" ]]; then
  report_error "missing section coverage file: ${SECTION_MATRIX}"
else
  if ! diff -u "${SECTION_MATRIX}" "${TMP_MATRIX}" > "${TMP_DIFF}"; then
    report_error "section coverage file out of date: ${SECTION_MATRIX} (run scripts/generate-doc-section-coverage.sh)"
    cat "${TMP_DIFF}" >&2 || true
  fi
fi

if (( HAS_ERRORS > 0 )); then
  exit 1
fi

echo "[doc-coverage] OK: migration-map and section coverage are complete and up to date."
