# CI/CD Pipeline — All Components

This document defines the build, test, and deployment pipeline for every component in the Harmonia ecosystem. All builds use Bazel. Deployment targets differ by component.

---

## Overview

| Component | Build Tool | Deploy Target | Trigger |
|-----------|-----------|---------------|---------|
| Harmonia (agent) | Bazel + Cargo | pkgsrc/pkgin (NetBSD) | Tag `harmonia-v*` |
| OS4-iOS | Bazel (rules_apple) | TestFlight → App Store | Tag `ios-v*` |
| OS4-Android | Bazel (rules_android) | Google Play (internal → production) | Tag `android-v*` |
| harmoniislib | Bazel (rules_rust) | Consumed by iOS/Android builds | On dependency change |
| OS4 (NetBSD) | Custom ISO builder | Manual flash / PXE boot | Tag `os4-v*` |

### Execution Phases

1. **Phase 1 (Now):** Manual from dev macOS. Build locally, upload manually.
2. **Phase 2 (After everything works):** GitHub Actions automates everything on tag push.

---

## Harmonia Agent — pkgsrc Distribution

### What Gets Distributed

The Harmonia agent is distributed via NetBSD's `pkgsrc` package system. A `pkgin install harmonia` gives the user:

1. **SBCL** (dependency, pulled automatically)
2. **Harmonia DNA** — the latest stable evolved S-expression source (`src/`)
3. **Pre-compiled Rust .so tools** — architecture-specific binaries (`lib/core/*.so`, `lib/backends/*.so`)
4. **Phoenix supervisor** — the PID 1 binary
5. **Configuration templates** — `config/*.sexp` with defaults
6. **Service scripts** — rc.d scripts for auto-start

### pkgsrc Package Definition

```makefile
# pkgsrc/wip/harmonia/Makefile

DISTNAME=       harmonia-${HARMONIA_VERSION}
CATEGORIES=     misc
MASTER_SITES=   https://github.com/harmonia-agent/harmonia/releases/download/v${HARMONIA_VERSION}/

MAINTAINER=     george@harmonia.dev
HOMEPAGE=       https://harmonia.dev/
COMMENT=        Recursive self-improving Lisp agent with Rust tools
LICENSE=        custom

DEPENDS+=       sbcl>=2.4:../../lang/sbcl
DEPENDS+=       rust>=1.75:../../lang/rust

USE_TOOLS+=     gmake pkg-config

# Build all Rust crates
do-build:
	cd ${WRKSRC} && cargo build --workspace --release

# Install layout matches NetBSD conventions
do-install:
	# Phoenix supervisor binary
	${INSTALL_PROGRAM} ${WRKSRC}/target/release/phoenix \
	    ${DESTDIR}${PREFIX}/bin/harmonia-phoenix

	# Rust .so tools → /usr/local/lib/harmonia/
	${INSTALL_LIB_DIR} ${DESTDIR}${PREFIX}/lib/harmonia
	${INSTALL_LIB_DIR} ${DESTDIR}${PREFIX}/lib/harmonia/core
	${INSTALL_LIB_DIR} ${DESTDIR}${PREFIX}/lib/harmonia/backends
	${INSTALL_LIB_DIR} ${DESTDIR}${PREFIX}/lib/harmonia/tools

	for so in ouroboros vault memory mqtt-client http s3-sync git-ops \
	          rust-forge cron-scheduler push-sns recovery browser fs; do \
	    ${INSTALL_LIB} ${WRKSRC}/target/release/lib$${so}.so \
	        ${DESTDIR}${PREFIX}/lib/harmonia/core/ ; \
	done

	${INSTALL_LIB} ${WRKSRC}/target/release/libharmonia_openrouter.so \
	    ${DESTDIR}${PREFIX}/lib/harmonia/backends/

	# Lisp source (the DNA) → /usr/local/share/harmonia/
	${INSTALL_DATA_DIR} ${DESTDIR}${PREFIX}/share/harmonia
	cd ${WRKSRC}/src && ${PAX} -rw . ${DESTDIR}${PREFIX}/share/harmonia/src/
	cd ${WRKSRC}/config && ${PAX} -rw . ${DESTDIR}${PREFIX}/share/harmonia/config/
	cd ${WRKSRC}/doc && ${PAX} -rw . ${DESTDIR}${PREFIX}/share/harmonia/doc/

	# rc.d service script
	${INSTALL_SCRIPT} ${FILESDIR}/harmonia.sh \
	    ${DESTDIR}${PREFIX}/share/examples/rc.d/harmonia

.include "../../mk/bsd.pkg.mk"
```

### rc.d Service Script

```sh
#!/bin/sh
# /etc/rc.d/harmonia

# PROVIDE: harmonia
# REQUIRE: DAEMON network
# KEYWORD: shutdown

. /etc/rc.subr

name="harmonia"
rcvar=$name
command="/usr/local/bin/harmonia-phoenix"
command_args="--config /usr/local/share/harmonia/config/agent.sexp"
pidfile="/var/run/${name}.pid"

# Phoenix is PID 1 for the agent — it manages SBCL internally
start_precmd="harmonia_prestart"

harmonia_prestart()
{
    # Ensure state directories exist
    mkdir -p /var/harmonia/state
    mkdir -p /var/harmonia/memory
    mkdir -p /var/harmonia/vault
    mkdir -p /var/harmonia/logs

    # Set library path for Rust .so tools
    export LD_LIBRARY_PATH="/usr/local/lib/harmonia/core:/usr/local/lib/harmonia/backends:$LD_LIBRARY_PATH"
    export HARMONIA_LIB_PATH="/usr/local/lib/harmonia"
    export HARMONIA_SRC_PATH="/usr/local/share/harmonia/src"
}

load_rc_config $name
run_rc_command "$@"
```

### Versioning Strategy (Genomic + Epigenetic)

The package version tracks two things:

1. **Genomic version** — Lisp source + policy (S-expressions). Architecture-neutral.
2. **Epigenetic/runtime version** — compiled Rust .so set + runtime expression artifacts. Architecture-specific (aarch64, x86_64).

When the agent evolves and reaches a stable state:

```
Agent evolves → runs validation → harmonic score improves →
  Agent commits genomic updates to git →
  CI builds new .so binaries for all architectures →
  CI creates pkgsrc release →
  pkgin update pulls latest stable version →
  Phoenix restarts with new genomic + epigenetic/runtime bundle
```

### Package Update Flow

```
User: pkgin update && pkgin upgrade harmonia

pkgin fetches:
  - New Lisp source/policy files (genomic) → /usr/local/share/harmonia/src/
  - New .so binaries (runtime instrument layer) → /usr/local/lib/harmonia/
  - service harmonia restart (Phoenix picks up changes)
```

### Architecture Support

| Architecture | NetBSD Target | Binary Suffix |
|-------------|---------------|---------------|
| ARM64 | evbarm-aarch64 | `harmonia-*-aarch64.tar.gz` |
| x86_64 | amd64 | `harmonia-*-x86_64.tar.gz` |
| ARM (32-bit) | evbarm-earmv7hf | `harmonia-*-armv7.tar.gz` (future) |

### Build Script (Manual → CI)

```bash
#!/bin/bash
# scripts/build_pkgsrc.sh — builds release tarballs for pkgsrc

set -e

VERSION=$1
if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    exit 1
fi

ARCH=$(uname -m)  # aarch64, x86_64

# 1. Build all Rust crates in release mode
cargo build --workspace --release

# 2. Create distribution tarball
DIST="harmonia-${VERSION}"
mkdir -p "$DIST/bin" "$DIST/lib/core" "$DIST/lib/backends" "$DIST/src" "$DIST/config" "$DIST/doc"

# Phoenix binary
cp target/release/phoenix "$DIST/bin/harmonia-phoenix"

# Core .so files
for so in ouroboros vault memory mqtt_client http s3_sync git_ops \
          rust_forge cron_scheduler push_sns recovery browser fs; do
    cp target/release/lib${so}.so "$DIST/lib/core/" 2>/dev/null || true
done

# Backend .so
cp target/release/libharmonia_openrouter.so "$DIST/lib/backends/"

# DNA (Lisp source)
cp -r src/ "$DIST/src/"
cp -r config/ "$DIST/config/"
cp -r doc/ "$DIST/doc/"

# Tarball
tar czf "harmonia-${VERSION}-${ARCH}.tar.gz" "$DIST"
rm -rf "$DIST"

echo "Built: harmonia-${VERSION}-${ARCH}.tar.gz"
```

---

## GitHub Actions — Unified CI/CD

### Harmonia Agent Release

```yaml
# .github/workflows/harmonia-release.yml

name: Harmonia Release → pkgsrc

on:
  push:
    tags: ['harmonia-v*']

jobs:
  build-netbsd:
    strategy:
      matrix:
        arch: [aarch64, x86_64]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Cross-compile for NetBSD
        # Uses cross-compilation via cargo + NetBSD sysroot
        # Or NetBSD VM via QEMU for native build
        run: |
          VERSION=${GITHUB_REF_NAME#harmonia-v}
          # Cross-build Rust for NetBSD target
          rustup target add ${{ matrix.arch }}-unknown-netbsd
          cargo build --workspace --release --target ${{ matrix.arch }}-unknown-netbsd
          bash scripts/build_pkgsrc.sh $VERSION

      - name: Upload release artifact
        uses: softprops/action-gh-release@v1
        with:
          files: harmonia-*-${{ matrix.arch }}.tar.gz

  update-pkgsrc:
    needs: build-netbsd
    runs-on: ubuntu-latest
    steps:
      - name: Update pkgsrc Makefile
        run: |
          VERSION=${GITHUB_REF_NAME#harmonia-v}
          # Update DISTNAME version in pkgsrc Makefile
          # Submit PR to pkgsrc-wip or pkgsrc repository
          echo "pkgsrc update for harmonia-$VERSION"
```

### Release Workflow Summary

```
Developer pushes tag:
  ios-v1.0.0     → builds iOS → uploads to TestFlight
  android-v1.0.0 → builds Android → uploads to Play Store internal
  harmonia-v1.0.0 → builds NetBSD binaries → creates GitHub release → updates pkgsrc

Promote to production:
  TestFlight → App Store: manual approval in App Store Connect
  Play Store internal → production: promote track via API or console
  pkgsrc: package available after pkgsrc tree update
```

---

## Secrets Management (GitHub)

All secrets are stored in GitHub repository secrets. Never in code.

| Secret | Used By | Description |
|--------|---------|-------------|
| `IOS_SIGNING_CERT_P12` | iOS CI | Apple signing certificate |
| `IOS_SIGNING_CERT_PASSWORD` | iOS CI | Certificate password |
| `IOS_PROVISIONING_PROFILE` | iOS CI | Provisioning profile |
| `APP_STORE_API_KEY` | iOS CI | App Store Connect API key (.p8) |
| `APP_STORE_API_KEY_ID` | iOS CI | Key ID |
| `APP_STORE_API_ISSUER_ID` | iOS CI | Issuer ID |
| `ANDROID_KEYSTORE` | Android CI | Release keystore (.jks, base64) |
| `ANDROID_KEYSTORE_PASSWORD` | Android CI | Keystore password |
| `ANDROID_KEY_ALIAS` | Android CI | Key alias |
| `GOOGLE_PLAY_SERVICE_ACCOUNT_JSON` | Android CI | Play Store API credentials |
