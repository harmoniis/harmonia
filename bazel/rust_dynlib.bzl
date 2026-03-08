"""Build Rust tools as dynamic libraries (.so) for SBCL CFFI."""

load("@rules_rust//rust:defs.bzl", "rust_shared_library")

def harmonia_rust_tool(name, srcs = None, deps = None, **kwargs):
    """Build a Harmonia Rust tool as a cdylib for CFFI loading."""
    rust_shared_library(
        name = name,
        srcs = srcs or native.glob(["src/**/*.rs"]),
        crate_type = "cdylib",
        deps = deps or [],
        **kwargs
    )
