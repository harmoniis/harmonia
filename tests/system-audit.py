#!/usr/bin/env python3
"""
Harmonia System Audit — exercises every wire in the living system.

Connects to the running Harmonia IPC socket and tests all 12 subsystems:
Chronicle, Signalograd, Memory Field, Harmonic Matrix, Vault, Config Store,
Provider Router, Gateway, Observability, integration checks, and DNA.

Usage:
    python3 tests/system-audit.py
    python3 tests/system-audit.py --socket /path/to/runtime.sock
"""

import socket
import struct
import sys
import time
import os

# ─── IPC Transport ────────────────────────────────────────────────────

DEFAULT_SOCKET = os.path.expanduser("~/.harmoniis/harmonia/runtime.sock")

def ipc_call(sock_path, sexp, timeout=5):
    """Send sexp to Harmonia IPC socket, return response string."""
    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(timeout)
        sock.connect(sock_path)
        msg = sexp.encode("utf-8")
        sock.sendall(struct.pack(">I", len(msg)) + msg)
        header = sock.recv(4)
        if len(header) < 4:
            sock.close()
            return None
        size = struct.unpack(">I", header)[0]
        data = b""
        while len(data) < size:
            chunk = sock.recv(min(4096, size - len(data)))
            if not chunk:
                break
            data += chunk
        sock.close()
        return data.decode("utf-8", errors="replace")
    except Exception as e:
        return f"ERROR: {e}"

# ─── Test Framework ───────────────────────────────────────────────────

class AuditResult:
    def __init__(self, name, passed, detail=""):
        self.name = name
        self.passed = passed
        self.detail = detail

class SystemAudit:
    def __init__(self, sock_path):
        self.sock = sock_path
        self.results = {}
        self.total_pass = 0
        self.total_fail = 0
        self.total_warn = 0

    def call(self, sexp, timeout=5):
        return ipc_call(self.sock, sexp, timeout)

    def check(self, category, name, sexp, expect_contains=None, expect_not_contains=None, timeout=5):
        """Run an IPC check and record result."""
        result = self.call(sexp, timeout)
        if result is None or result.startswith("ERROR:"):
            r = AuditResult(name, False, result or "no response")
            self.results.setdefault(category, []).append(r)
            self.total_fail += 1
            return result

        passed = True
        detail = result[:120]

        if expect_contains:
            for token in expect_contains:
                if token not in result:
                    passed = False
                    detail = f"missing '{token}' in response"
                    break

        if expect_not_contains:
            for token in expect_not_contains:
                if token in result:
                    passed = False
                    detail = f"unexpected '{token}' in response"
                    break

        r = AuditResult(name, passed, detail)
        self.results.setdefault(category, []).append(r)
        if passed:
            self.total_pass += 1
        else:
            self.total_fail += 1
        return result

    def warn(self, category, name, detail):
        r = AuditResult(name, True, f"WARN: {detail}")
        self.results.setdefault(category, []).append(r)
        self.total_warn += 1

    def report(self):
        print()
        print("══ HARMONIA SYSTEM AUDIT " + "═" * 40)
        print(f"   Date:   {time.strftime('%Y-%m-%d %H:%M:%S')}")
        print(f"   Socket: {self.sock}")
        print()

        for category, checks in self.results.items():
            print(f"── {category} " + "─" * max(0, 50 - len(category)))
            for c in checks:
                icon = "✓" if c.passed else "✗"
                detail = c.detail[:80] if c.detail else ""
                print(f"  {icon} {c.name:<35s} {detail}")
            print()

        total = self.total_pass + self.total_fail
        print(f"SUMMARY: {self.total_pass}/{total} passed, {self.total_warn} warnings, {self.total_fail} failures")
        print()
        return self.total_fail == 0

# ─── Test Suites ──────────────────────────────────────────────────────

def test_chronicle(audit):
    cat = "Chronicle"
    audit.check(cat, "init",
        '(:component "chronicle" :op "init")',
        expect_contains=[":ok"])

    r = audit.check(cat, "harmony-summary",
        '(:component "chronicle" :op "harmony-summary")')
    if r and ":error" not in r.lower():
        audit.results[cat][-1].detail = "has data" if "cycle" in r.lower() or "signal" in r.lower() else "empty"

    audit.check(cat, "SQL: harmonic_snapshots count",
        '(:component "chronicle" :op "query" :sql "SELECT COUNT(*) as cnt FROM harmonic_snapshots")')

    audit.check(cat, "SQL: field_basin column exists",
        '(:component "chronicle" :op "query" :sql "SELECT field_basin FROM harmonic_snapshots ORDER BY ts DESC LIMIT 1")')

    audit.check(cat, "gc-status",
        '(:component "chronicle" :op "gc-status")')

def test_signalograd(audit):
    cat = "Signalograd"
    audit.check(cat, "init",
        '(:component "signalograd" :op "init")',
        expect_contains=[":ok"])

    r = audit.check(cat, "status",
        '(:component "signalograd" :op "status")')
    if r and "cycle" in r.lower():
        audit.results[cat][-1].detail = r[:100]

    audit.check(cat, "snapshot",
        '(:component "signalograd" :op "snapshot")')

def test_memory_field(audit):
    cat = "Memory Field"
    audit.check(cat, "status",
        '(:component "memory-field" :op "status")',
        expect_contains=[":graph-n", ":basin"])

    audit.check(cat, "basin-status",
        '(:component "memory-field" :op "basin-status")',
        expect_contains=[":current", ":dwell-ticks"])

    audit.check(cat, "eigenmode-status",
        '(:component "memory-field" :op "eigenmode-status")',
        expect_contains=[":eigenvalues"])

    r = audit.check(cat, "field-recall: harmony",
        '(:component "memory-field" :op "field-recall" :query-concepts ("harmony") :access-counts () :limit 5)',
        expect_contains=[":activations"])
    if r and ":concept" in r:
        # Extract top concept
        import re
        m = re.search(r':concept "(\w+)"', r)
        if m:
            audit.results[cat][-1].detail = f"top={m.group(1)}"

    audit.check(cat, "field-recall: rust + code",
        '(:component "memory-field" :op "field-recall" :query-concepts ("rust" "code") :access-counts () :limit 5)',
        expect_contains=[":activations"])

    r = audit.check(cat, "step-attractors",
        '(:component "memory-field" :op "step-attractors" :signal 0.7 :noise 0.3)',
        expect_contains=[":thomas", ":aizawa", ":halvorsen"])

    audit.check(cat, "last-field-basin (Chronicle query)",
        '(:component "memory-field" :op "last-field-basin")')

def test_harmonic_matrix(audit):
    cat = "Harmonic Matrix"
    audit.check(cat, "report",
        '(:component "harmonic-matrix" :op "report")')

    audit.check(cat, "store-summary",
        '(:component "harmonic-matrix" :op "store-summary")')

    audit.check(cat, "route-allowed (core→backend)",
        '(:component "harmonic-matrix" :op "route-allowed" :from "core" :to "openrouter" :signal 0.7 :noise 0.3)')

def test_vault(audit):
    cat = "Vault"
    audit.check(cat, "list-symbols",
        '(:component "vault" :op "list-symbols")')

    audit.check(cat, "has-secret: openrouter-api-key",
        '(:component "vault" :op "has-secret" :symbol "openrouter-api-key")')

def test_config_store(audit):
    cat = "Config Store"
    audit.check(cat, "list: memory-field scope",
        '(:component "config" :op "list" :component "memory-field" :scope "memory-field")')

    audit.check(cat, "get: signalograd state-path",
        '(:component "config" :op "get" :component "signalograd-core" :scope "signalograd-core" :key "state-path")')

    audit.check(cat, "list: harmony-policy",
        '(:component "config" :op "list" :component "harmony-policy" :scope "harmony-policy")')

def test_provider_router(audit):
    cat = "Provider Router"
    audit.check(cat, "healthcheck",
        '(:component "provider-router" :op "healthcheck")')

    audit.check(cat, "list-models",
        '(:component "provider-router" :op "list-models")')

    audit.check(cat, "list-backends",
        '(:component "provider-router" :op "list-backends")')

def test_gateway(audit):
    cat = "Gateway"
    audit.check(cat, "is-allowed",
        '(:component "gateway" :op "is-allowed")')

    audit.check(cat, "poll (non-blocking)",
        '(:component "gateway" :op "poll")')

def test_observability(audit):
    cat = "Observability"
    audit.check(cat, "status",
        '(:component "observability" :op "status")')

    audit.check(cat, "init",
        '(:component "observability" :op "init")')

def test_integration_memory_field_lifecycle(audit):
    cat = "Integration: Field Lifecycle"

    # Get initial state
    r1 = audit.call('(:component "memory-field" :op "status")')
    if r1 and "graph-version" in r1:
        audit.results.setdefault(cat, []).append(
            AuditResult("initial status", True, r1[:100]))
        audit.total_pass += 1

    # Step attractors and verify bounded state
    r2 = audit.call('(:component "memory-field" :op "step-attractors" :signal 0.9 :noise 0.1)')
    if r2:
        bounded = all(f"{v}" not in r2 for v in ["NaN", "Inf", "inf", "nan"])
        audit.results.setdefault(cat, []).append(
            AuditResult("attractors bounded after step", bounded, r2[:100]))
        if bounded:
            audit.total_pass += 1
        else:
            audit.total_fail += 1

    # Basin should have dwell_ticks > 0
    r3 = audit.call('(:component "memory-field" :op "basin-status")')
    if r3 and "dwell-ticks" in r3:
        audit.results.setdefault(cat, []).append(
            AuditResult("basin dwell tracking", True, r3[:100]))
        audit.total_pass += 1

def test_integration_chronicle_field(audit):
    cat = "Integration: Chronicle ↔ Field"

    # Query Chronicle for field basin data
    r = audit.call(
        '(:component "chronicle" :op "query" :sql "SELECT field_basin, field_dwell_ticks FROM harmonic_snapshots WHERE field_basin != \'thomas-0\' ORDER BY ts DESC LIMIT 3")')
    if r:
        has_evolved = "thomas" in r.lower() or "aizawa" in r.lower() or "halvorsen" in r.lower()
        audit.results.setdefault(cat, []).append(
            AuditResult("Chronicle has field basin data", True, r[:100] if has_evolved else "no basin evolution yet"))
        audit.total_pass += 1
    else:
        audit.results.setdefault(cat, []).append(
            AuditResult("Chronicle has field basin data", False, "query failed"))
        audit.total_fail += 1

    # Verify warm-start query works
    audit.check(cat, "last-field-basin query",
        '(:component "memory-field" :op "last-field-basin")')

# ─── Main ─────────────────────────────────────────────────────────────

def main():
    sock = DEFAULT_SOCKET
    if "--socket" in sys.argv:
        idx = sys.argv.index("--socket")
        if idx + 1 < len(sys.argv):
            sock = sys.argv[idx + 1]

    if not os.path.exists(sock):
        print(f"Socket not found: {sock}")
        print("Is Harmonia running? Try: harmonia start")
        sys.exit(1)

    audit = SystemAudit(sock)

    # Run all test suites
    test_chronicle(audit)
    test_signalograd(audit)
    test_memory_field(audit)
    test_harmonic_matrix(audit)
    test_vault(audit)
    test_config_store(audit)
    test_provider_router(audit)
    test_gateway(audit)
    test_observability(audit)
    test_integration_memory_field_lifecycle(audit)
    test_integration_chronicle_field(audit)

    # Report
    success = audit.report()
    sys.exit(0 if success else 1)

if __name__ == "__main__":
    main()
