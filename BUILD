# Harmonia — Lisp Agent + lib (core, backends, tools)
# Build all: bazel build //harmonia/lib/...

package(default_visibility = ["//visibility:public"])

filegroup(
    name = "lib",
    srcs = [],
    data = [
        # Core — essential infrastructure
        "//harmonia/lib/core/phoenix:phoenix",
        "//harmonia/lib/core/ouroboros:ouroboros-so",
        "//harmonia/lib/core/vault:vault-so",
        "//harmonia/lib/core/memory:memory-so",
        "//harmonia/lib/core/mqtt-client:mqtt-client",
        "//harmonia/lib/core/http:http-so",
        "//harmonia/lib/core/s3-sync:s3-sync",
        "//harmonia/lib/core/git-ops:git-ops",
        "//harmonia/lib/core/rust-forge:rust-forge",
        "//harmonia/lib/core/cron-scheduler:cron-scheduler",
        "//harmonia/lib/core/push-sns:push-sns",
        "//harmonia/lib/core/recovery:recovery-so",
        "//harmonia/lib/core/browser:browser-so",
        "//harmonia/lib/core/fs:fs-so",

        # Backends
        "//harmonia/lib/backends/openrouter-backend:openrouter-backend",

        # Tools — optional plugins
        "//harmonia/lib/tools/pgp-identity:pgp-identity",
        "//harmonia/lib/tools/webcash-wallet:webcash-wallet",
        "//harmonia/lib/tools/social:social",
    ],
)
