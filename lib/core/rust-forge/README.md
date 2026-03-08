# harmonia-rust-forge

## Purpose

Runtime Rust compiler that builds Rust source packages into `.so` shared libraries. Enables the agent to write, compile, and hot-load new crates without a full rebuild cycle.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_rust_forge_version` | `() -> *const c_char` | Version string |
| `harmonia_rust_forge_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_rust_forge_build_package` | `(crate_dir: *const c_char, output_dir: *const c_char) -> i32` | Compile crate at path to cdylib |
| `harmonia_rust_forge_last_error` | `() -> *mut c_char` | Last error (includes compiler stderr) |
| `harmonia_rust_forge_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

Requires `cargo` and a Rust toolchain on `$PATH`. No special env vars.

## Usage from Lisp

```lisp
;; Compile a crate to .so
(let ((rc (cffi:foreign-funcall "harmonia_rust_forge_build_package"
            :string "/tmp/harmonia/patches/new-tool/"
            :string "/tmp/harmonia/lib/" :int)))
  (when (= rc 0)
    (cffi:foreign-funcall "harmonia_gateway_register"
      :string "new-tool"
      :string "/tmp/harmonia/lib/libharmonia_new_tool.so"
      :string "" :int)))
```

## Self-Improvement Notes

- Invokes `cargo build --release` with `--target-dir` set to output_dir.
- The compiled `.so` lands in `output_dir/release/lib<crate_name>.so`.
- Error output includes full compiler stderr for LLM-driven debugging.
- This is the key enabler for ouroboros self-healing: write patch -> forge compile -> gateway reload.
- To add caching: hash the source directory and skip rebuild if unchanged.
- To add cross-compilation: pass `--target` flag for Android/iOS builds.
