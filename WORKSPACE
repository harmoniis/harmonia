workspace(name = "harmonia")

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")

# ============================================================================
# Rust Toolchain
# ============================================================================
http_archive(
    name = "rules_rust",
    sha256 = "36ab8f9facae745c9c9c1b33d225623d976e78f2cc3f729b7973d8c20934ab95",
    urls = ["https://github.com/bazelbuild/rules_rust/releases/download/0.31.0/rules_rust-v0.31.0.tar.gz"],
)

load("@rules_rust//rust:repositories.bzl", "rules_rust_dependencies", "rust_register_toolchains")

rules_rust_dependencies()

rust_register_toolchains(
    edition = "2021",
    versions = ["1.93.0"],
)

# ============================================================================
# Crate Universe (External Rust dependencies)
# ============================================================================
load("@rules_rust//crate_universe:defs.bzl", "crates_repository")

crates_repository(
    name = "crate_index",
    cargo_lockfile = "//:Cargo.lock",
    lockfile = "//:Cargo.Bazel.lock",
    manifests = ["//:Cargo.toml"],
)

load("@crate_index//:defs.bzl", "crate_repositories")

crate_repositories()

# ============================================================================
# Scope: Harmonia agent (standalone workspace)
#   //lib/core/*      → Agent's core infrastructure
#   //lib/backends/*  → LLM providers
#   //lib/tools/*     → On-demand capability tools
#   //lib/frontends/* → Hot-pluggable communication channels
# ============================================================================
