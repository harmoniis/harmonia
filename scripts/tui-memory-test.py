#!/usr/bin/env python3
"""Integration test: drive Harmonia from the TUI socket and verify memory.

Connects to /var/folders/.../T/harmonia/harmonia.sock (the gateway frontend
socket the TUI uses), sends prompts as lines, reads responses, and asserts
that memory persists across prompts.

Requires: harmonia-runtime running, SBCL agent running.
"""

import os
import socket
import sys
import time

SOCKET_DIR = "/var/folders/3y/xmp5j1xj3ldcxxm2p_xqwwgc0000gn/T/harmonia"
SOCKET_PATH = os.path.join(SOCKET_DIR, "harmonia.sock")

PROMPT_TIMEOUT_S = 90.0  # generous: free-tier OpenRouter models can be slow


def open_session():
    if not os.path.exists(SOCKET_PATH):
        print(f"FAIL: socket not present: {SOCKET_PATH}", file=sys.stderr)
        sys.exit(2)
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(SOCKET_PATH)
    s.settimeout(PROMPT_TIMEOUT_S)
    return s


def send_prompt(sock, text):
    """Send one line, then read response lines until quiescent for ~3s."""
    sock.sendall((text + "\n").encode("utf-8"))
    chunks = []
    f = sock.makefile("rb")
    sock.settimeout(8.0)  # idle window after which we declare the response done
    deadline = time.time() + PROMPT_TIMEOUT_S
    while True:
        try:
            line = f.readline()
        except socket.timeout:
            break
        if not line:
            break
        chunks.append(line.decode("utf-8", errors="replace").rstrip("\n"))
        if time.time() > deadline:
            break
    return chunks


def join_response(chunks):
    return "\n".join(c for c in chunks if c.strip())


def assert_contains_any(needle_keywords, haystack, label):
    """Soft assertion: report whether any of the keywords appear in the response."""
    haystack_lower = haystack.lower()
    hits = [k for k in needle_keywords if k.lower() in haystack_lower]
    status = "✓" if hits else "✗"
    print(f"  {status} {label} (matched: {hits if hits else 'none'})")
    return bool(hits)


def main():
    print("── TUI memory integration test ─────────────────────────────")
    sock = open_session()
    print(f"connected to {SOCKET_PATH}")

    test_results = []

    # Prompt 1: plant a specific fact in memory.
    p1 = (
        "Please remember this for me as a fact about my work: "
        "I am studying how Riemann zeta zeros relate to graph Laplacian eigenvalues "
        "in Harmonia's memory field. Confirm with one short sentence."
    )
    print("\n[1] Planting fact about Riemann–Laplacian study…")
    r1 = send_prompt(sock, p1)
    body1 = join_response(r1)
    print(f"    response ({len(body1)} chars): {body1[:240]}…" if len(body1) > 240 else f"    response: {body1}")
    test_results.append(("planted-fact-acknowledged",
        assert_contains_any(["riemann", "zeta", "laplacian", "remember", "noted"], body1,
                             "response acknowledges the planted fact")))

    time.sleep(2)  # let the auto-store flush before the recall prompt

    # Prompt 2: recall test — generic question that requires memory.
    p2 = "What did I tell you I'm working on? Use only memory, no guessing."
    print("\n[2] Asking what the user is working on (memory recall)…")
    r2 = send_prompt(sock, p2)
    body2 = join_response(r2)
    print(f"    response ({len(body2)} chars): {body2[:240]}…" if len(body2) > 240 else f"    response: {body2}")
    test_results.append(("recall-finds-fact",
        assert_contains_any(["riemann", "zeta", "laplacian", "memory field", "harmonia"], body2,
                             "recall returns the planted fact")))

    time.sleep(2)

    # Prompt 3: introspection — exercises memory-recall scoring.
    p3 = "List up to three concepts you remember from this session, ranked by relevance."
    print("\n[3] Asking for top-3 remembered concepts (recall + ranking)…")
    r3 = send_prompt(sock, p3)
    body3 = join_response(r3)
    print(f"    response ({len(body3)} chars): {body3[:240]}…" if len(body3) > 240 else f"    response: {body3}")
    test_results.append(("ranked-recall-mentions-fact",
        assert_contains_any(["riemann", "zeta", "laplacian", "eigen", "memory"], body3,
                             "ranked recall surfaces a related concept")))

    sock.close()

    # Summary.
    print("\n── SUMMARY ─────────────────────────────────────────────────")
    passes = sum(1 for _, ok in test_results if ok)
    fails = sum(1 for _, ok in test_results if not ok)
    for name, ok in test_results:
        print(f"  {'PASS' if ok else 'FAIL'}  {name}")
    print(f"\nresult: {passes} pass, {fails} fail")
    sys.exit(0 if fails == 0 else 1)


if __name__ == "__main__":
    main()
