# FreeBSD pkg pipeline

This directory holds everything the release CI needs to produce
`harmonia-<version>.pkg` for FreeBSD 14 amd64. The `.pkg` is attached to
the matching GitHub Release; cluster pots install it during bake via
`pkg add` against the GitHub asset URL.

No private pkg repo, no signing key, no S3 bucket. Just the .pkg in a
GitHub release.

## Layout

| File                  | Purpose                                                  |
| --------------------- | -------------------------------------------------------- |
| `+MANIFEST.tmpl`      | UCL manifest with `__VERSION__` placeholder.             |
| `+PRE_INSTALL`        | Creates the unprivileged `harmonia` user.                |
| `+POST_INSTALL`       | Lays down `/var/db/harmonia` and copies the sample config. |
| `+PRE_DEINSTALL`      | Stops the service before files vanish (never wipes state).|
| `rc.d/harmonia`       | FreeBSD-standard rc.d service (lands at `/usr/local/etc/rc.d/`). |
| `config.toml.sample`  | Skeleton config installed at `/usr/local/share/harmonia`. |

The CI workflow lives at `.github/workflows/release-freebsd-pkg.yml`.

## Triggering a release

Tag the repo with `harmonia-v<semver>` matching the harmonia crate version
in `Cargo.toml` and push:

```
git tag harmonia-v0.2.1
git push origin harmonia-v0.2.1
```

The workflow also exposes `workflow_dispatch` for one-off builds with an
explicit version string.

What the workflow does:

1. Spins up a FreeBSD 14.1 VM (`vmactions/freebsd-vm@v1`).
2. `cargo build --release -p harmonia --bin harmonia`.
3. Stages the rootfs (`/usr/local/bin/harmonia`, rc.d, sample config, license).
4. `pkg create -M +MANIFEST -r stage -o out` → `harmonia-<version>.pkg`.
5. Generates a SHA256 sidecar.
6. Attaches both files to the GitHub Release for the tag.

## Installing on a FreeBSD host (developer / smoke test)

```
# Pinned version
fetch -o /tmp/harmonia.pkg \
    https://github.com/harmoniis/harmonia/releases/download/harmonia-v0.2.1/harmonia-0.2.1.pkg
pkg add /tmp/harmonia.pkg

# Or latest
LATEST=$(fetch -q -o - https://api.github.com/repos/harmoniis/harmonia/releases/latest \
    | grep '"tag_name"' | head -1 | sed 's/.*"harmonia-v\(.*\)".*/\1/')
fetch -o /tmp/harmonia.pkg \
    https://github.com/harmoniis/harmonia/releases/download/harmonia-v${LATEST}/harmonia-${LATEST}.pkg
pkg add /tmp/harmonia.pkg

service harmonia onestart
service harmonia status
```

State lives at `/var/db/harmonia`. Removing the package preserves state
intentionally — to forget the device's identity completely, delete the
wallet directory by hand:

```
pkg delete -y harmonia
rm -rf /var/db/harmonia      # only if you want to wipe device identity
```

## Cluster pots

`harmonia-infra-provision/pot-images/harmonia-agent/install.sh` is the
pot-bake script. It is invoked once per bake from the pot orchestrator
(`pot-images/build.sh`), runs inside `jexec`, and does exactly what the
manual install above does — fetches the release asset, verifies the
SHA256, runs `pkg add`. The pot snapshot then captures the installed
image, and Nomad schedules clones of that snapshot onto cluster nodes.

The state directory `/var/db/harmonia` is mounted from the persistent
Ceph-RBD volume defined in `terraform/modules/provisioner/templates/`
so the wallet seed (`/var/db/harmonia/wallet/seed`) survives both
`pkg upgrade harmonia` and a full pot re-bake. The device's PGP
fingerprint — the thing other devices trust in the multi-device trust
web — is therefore stable across releases. See the deploy-pipeline plan
section W3.1 for the load-bearing invariant.

## What this design does NOT need

- **No private pkg repo.** Cluster pots `pkg add` directly from the
  GitHub asset URL. End users on their own FreeBSD machines do the same.
- **No signing keypair.** GitHub releases are served over HTTPS from
  github.com; trust is bootstrapped from `ca_root_nss`. The SHA256
  sidecar lets the pot verify the download end-to-end before installing.
- **No S3, no Cloudflare frontage.** GitHub is the only distribution
  surface. Egress + retention is GitHub's problem.

## What this design does NOT yet do

- **No `pkg install harmonia` from a shell against a repo URL.** The
  `pkg add` flow is what runs during pot bake. If you later want
  `pkg install harmonia` to work for one-off operator boxes — for
  example, on the FreeBSD jumpbox — you can publish a thin pkg repo
  metadata branch on GitHub Pages and point `harmonia.repo.conf` at
  `pkg+https://harmoniis.github.io/harmonia/freebsd:14:amd64`. That is
  a separate session of work; the .pkg artifact does not change.
