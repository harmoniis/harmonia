# SBCL — Runtime for Harmonia

SBCL (Steel Bank Common Lisp) is a high-performance, ANSI-compliant implementation of Common Lisp. It is the runtime that powers Harmonia — the recursive self-improving agent.

## Why SBCL

### S-Expressions and Homoiconicity

The boundary between "code" and "data" is non-existent:

- **S-Expressions:** Everything in Lisp is an S-expression — nested lists like `(function argument1 argument2)`.
- **Homoiconicity:** Source code is written in the same data structure the language uses internally. Code IS data. The agent can analyze, transform, and rewrite its own source as data structures — the basis of self-modification.
- **Macros:** Meta-programming is first-class. The agent can write macros that generate code patterns it discovers are harmonically efficient.

### Self-Modification is Natural

`(read-from-string ...)` parses source code into data. `(eval ...)` executes data as code. `(compile ...)` compiles to native machine code. `(load ...)` loads compiled code into the running image. The entire self-modification loop is built into the language.

### Native Compilation

SBCL compiles Common Lisp to native machine code (x86, ARM, RISC-V, etc.). Not interpreted. Not bytecode. Native. This matters for the agent's core loop performance.

---

## Quicklisp Dependencies (Minimal)

Harmonia uses Quicklisp only for CFFI. No serialization libraries. No HTTP clients. No MQTT libraries in Lisp.

| Package | Purpose |
|---------|---------|
| `cffi` | Load Rust `.so` dynamic libraries via dlopen/dlsym |
| `bordeaux-threads` | Threading for concurrent tool invocations |

That is it. No `cl-json`. No `cl-mqtt`. No `dexador`. No `jonathan`. No `shasht`.

The agent does not use Lisp libraries for I/O, networking, or serialization. All external interaction goes through Rust dynamic libraries loaded via CFFI. Lisp stays pure — orchestration, logic, pattern detection, self-modification.

---

## The Serialization Contract

Lisp speaks s-expressions natively. This requires zero libraries:

- **Output:** `(format nil "~S" data)` — built into SBCL, prints any Lisp data as an s-expression string
- **Input:** `(read-from-string str)` — built into SBCL, parses an s-expression string into Lisp data

The Rust `.so` tools contain a lightweight s-expression parser that translates between s-expressions and JSON at the MQTT boundary. Lisp never touches JSON.

---

## Setup on NetBSD

1. **Install SBCL:** `pkgin install sbcl`
2. **Install Quicklisp:**
   - Download `quicklisp.lisp` from quicklisp.org
   - `sbcl --load quicklisp.lisp`
   - `(quicklisp-quickstart:install)`
3. **Load CFFI:** `(ql:quickload :cffi)` — fetches and compiles CFFI
4. **Load tools:** CFFI loads Rust `.so` files via `(cffi:load-foreign-library ...)`

---

## SBCL Core Image

After bootstrap, Harmonia dumps a core image to disk. This image contains:

- All loaded Lisp source (compiled to native code)
- All CFFI foreign library registrations
- The current state machine state
- Memory store contents

On restart, SBCL loads the core image and resumes instantly — no recompilation, no re-evaluation. This is part of the epigenetic layer (runtime expression), not the genomic source layer.

```lisp
;; Dump core image
(sb-ext:save-lisp-and-die "/var/harmonia/state/harmonia.core"
                          :toplevel #'harmonia:resume
                          :executable nil)

;; Resume from core image
;; sbcl --core /var/harmonia/state/harmonia.core
```
