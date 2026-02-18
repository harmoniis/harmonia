# Genesis Development Flow

**Purpose:** Build the initial Harmonia agent state, iOS app, and Android app together. Establish a closed loop for local testing before evolution is enabled. Deploy to TestFlight and Android device testing.

---

## 1. Overview

### Build Order

```
1. harmoniis backend + frontend (API for PGP trust)
2. rumqttd (MQTT broker with PGP auth)
3. Harmonia core tools (Rust .so)
4. Harmonia agent (SBCL + Lisp)
5. OS4-iOS app
6. OS4-Android app
7. Closed-loop local test
8. Deploy: TestFlight, Android device
```

### Closed Loop (No Evolution Until Validated)

The agent must pass a full local test cycle before evolution is enabled:

1. **Tools load** — All core .so tools load into SBCL without error
2. **Vault works** — OpenRouter API key injected correctly, never exposed to Lisp
3. **MQTT connects** — Agent connects to rumqttd with PGP auth
4. **First prompt** — User sends prompt via MQTT; agent selects model, calls OpenRouter, returns response
5. **Model selection** — Correct model for user prompt (kimi/grok for routine, opus for complex)
6. **Evolution disabled** — `evolution.enabled = false` until closed loop passes

---

## 2. MQTT: rumqttd + PGP-as-CA (Hybrid Web of Trust → TLS)

### Bridge: PGP as Certificate Authority

The PGP key is the **Root of Trust**. It is used as a Certificate Authority (CA) to issue X.509 certificates for MQTT devices. rumqttd (and Rustls) speak TLS/X.509 natively — they do not read `.gpg` or `.asc` files. The translation layer converts PGP Ed25519 keys into X.509 certificates.

| Concept | PGP World | TLS World |
|---------|-----------|-----------|
| Root of Trust | PGP Private Key | CA Certificate |
| Device Identity | PGP Public Key | Client Certificate (.pem) |
| Signing | PGP signs subkey/cert | CA signs client cert |
| Verification | PGP signature check | TLS cert chain validation |

### Phase 1: Provisioning (Pairing)

Before any MQTT connection, each device is provisioned:

1. **Extract Public Key:** Take the Ed25519 public key from the PGP identity.
2. **Wrap in X.509:** Create an X.509 certificate (.pem) containing that public key.
3. **Sign the Certificate:** Use the PGP private key (acting as CA) to sign this certificate. This "blesses" it as authentic.
4. **Distribute:**
   - **Broker** gets the PGP-derived CA certificate (`ca.pem`) — the trusted root.
   - **Client** gets its own unique certificate (`device.pem`) and private key (`device.key`).

**Translation Layer:** A small script or Rust tool (using `openssl` or crates like `x509-parser`, `rcgen`, `rustls-pemfile`) converts PGP Ed25519 key bits into X.509 containers. Rustls cannot natively read `.gpg` or `.asc` files.

```bash
# Example: provision-device.sh (conceptual)
# Input: PGP public key (from harmoniis trusted list or local)
# Output: device.pem, device.key (X.509 + Ed25519 keypair)
# The device cert is signed by the PGP-derived CA cert
./scripts/provision-device.sh --pgp-pub key.asc --label "ios-device-1" --out-dir ./certs/
```

### Phase 2: Connection Flow (Mutual TLS)

When a client connects to rumqttd on the TLS port:

1. **Server Proves Identity:**
   - Broker sends its certificate to the client.
   - Client checks: "Is this signed by the PGP-derived CA I trust?" If yes, connection is encrypted.

2. **Client Proves Identity:**
   - Broker requests the client's certificate (mTLS).
   - Client sends its cert and proves ownership via TLS handshake signature.
   - Broker checks: "Is this client cert signed by my trusted PGP root (ca_path)?"

3. **Encrypted Session:** Only if both sides pass the cryptographic check does MQTT start.

### rumqttd Configuration

```toml
# rumqttd.conf

[listeners]
default = "0.0.0.0:1883"   # Plain (dev only; no auth)

[listeners.tls]
tls = "0.0.0.0:8883"
verify_client_cert = true   # Enforce mTLS — client must present valid cert

# Broker's own cert (signed by PGP-CA or self-signed for dev)
cert_path = "/var/harmonia/certs/broker.pem"
key_path = "/var/harmonia/certs/broker.key"

# PGP-derived CA: the trusted root
# Only client certs signed by this CA are accepted
ca_path = "/var/harmonia/certs/pgp-ca.pem"
```

| Component | Responsibility |
|-----------|----------------|
| `ca_path` | PGP public key wrapped as X.509 CA cert. Loaded into rumqttd as the trusted root. |
| `verify_client_cert` | `true` — forces the pairing check. Client must present a cert signed by the CA. |
| Ed25519 | Same curve as PGP; math remains identical. |

### Why This Is Secure

- **No Passwords:** Authentication is purely cryptographic.
- **Identity Pinning:** Knowing the broker IP/port is insufficient; a valid cert signed by the PGP key is required.
- **Revocation:** Stop trusting a key → stop accepting certs signed by that identity. Update CA or use a CRL.

### The Translation Layer (Implementation)

**Rust crates:** `rcgen` (generate certs), `rustls-pemfile` (read/write PEM), `pgp` or `sequoia-pgp` (extract Ed25519 from PGP). Flow:

1. Load PGP public key, extract Ed25519 bytes.
2. Create X.509 CA cert from those bytes (or use PGP key to sign a self-issued CA cert).
3. For each device: generate Ed25519 keypair, create CSR or cert, sign with PGP private key.
4. Output: `ca.pem`, `broker.pem`, `broker.key`, `device-{id}.pem`, `device-{id}.key`.

**Provisioning service:** Can be part of harmoniis.com — `POST /api/harmonia/provision` accepts PGP identity proof, returns device cert + key (or a one-time download URL). Agent and mobile apps call this during pairing.

### Mutual Trust (Application Layer)

TLS ensures only trusted certs connect. Application-layer trust is still needed:

- **rumqttd** accepts any client with a cert signed by the PGP-CA.
- **iOS/Android** fetch the list of trusted agent fingerprints from `harmoniis.com/api/harmonia/auth/trusted` and only process MQTT messages from those identities (cert fingerprint or MQTT client-id mapping).
- **Agent** does the same for device identities.

---

## 3. Harmoniis API — Harmonia Endpoints

All Harmonia-specific endpoints are proxied by the frontend to the backend. Base URL: `https://harmoniis.com/api/harmonia/` (or `http://localhost:5173/api/harmonia/` in dev).

### 3.1 Get Trusted PGP Keys

**Purpose:** Agent, iOS, and Android fetch the list of trusted PGP public keys. Used for MQTT authentication.

**Endpoint:** `GET /api/harmonia/trusted`

**Request:** None (public endpoint, or optionally `X-Client-Fingerprint` for audit)

**Response:**
```json
{
  "trusted": [
    {
      "fingerprint": "ABC123...",
      "public_key": "-----BEGIN PGP PUBLIC KEY BLOCK-----...",
      "label": "agent-primary",
      "created_at": "2026-02-17T10:00:00Z"
    },
    {
      "fingerprint": "DEF456...",
      "public_key": "-----BEGIN PGP PUBLIC KEY BLOCK-----...",
      "label": "ios-device-1",
      "created_at": "2026-02-17T10:05:00Z"
    }
  ]
}
```

**Backend:** Stores trusted keys in DynamoDB. Keys are added via the auth flow.

---

### 3.2 Register PGP Identity (Prove Ownership)

**Purpose:** Agent or mobile app proves it owns a PGP key by signing a challenge. On success, the key is added to the trusted list.

**Endpoint:** `POST /api/harmonia/register`

**Request:**
```json
{
  "public_key": "-----BEGIN PGP PUBLIC KEY BLOCK-----...",
  "signature": "<base64 or ASCII-armored signature of challenge>",
  "challenge": "harmonia-register-<timestamp>-<nonce>",
  "label": "agent-primary"
}
```

**Flow:**
1. Client requests a challenge: `GET /api/harmonia/challenge?fingerprint=<hex>`
2. Server returns `{ "challenge": "harmonia-register-...", "expires_at": "..." }`
3. Client signs the challenge with its private key
4. Client POSTs to `/api/harmonia/register` with public_key, signature, challenge, label
5. Server verifies signature, adds to trusted list

**Response:**
```json
{
  "status": "registered",
  "fingerprint": "ABC123...",
  "label": "agent-primary"
}
```

---

### 3.3 Auth — Set Trusted PGPs (iOS/Android)

**Purpose:** User authenticates (e.g., via Harmoniis identity) and configures which PGP keys the app trusts. The agent's PGP must be in this list for the app to accept MQTT from it.

**Endpoint:** `POST /api/harmonia/auth/trusted`

**Request:** Requires Harmoniis session (cookie or bearer). User must be logged in.

```json
{
  "trusted_fingerprints": ["ABC123...", "DEF456..."],
  "device_id": "ios-uuid-..."
}
```

**Response:**
```json
{
  "status": "ok",
  "trusted_count": 2
}
```

**Endpoint:** `GET /api/harmonia/auth/trusted`

**Purpose:** Fetch the trusted list for the authenticated user's device.

**Response:** Same as `GET /api/harmonia/trusted` but filtered to keys the user has explicitly trusted for this device.

---

### 3.4 GraphQL (Existing)

All existing Harmoniis GraphQL operations are available. The frontend proxies `/api/graphql` → backend `/api/v1/graphql`. Harmonia-specific queries/mutations can be added under a `harmonia` namespace if needed.

---

## 4. Genesis Build Steps

### Step 1: harmoniis Backend + Frontend

```bash
cd harmoniis/backend
cargo build
# Set DYNAMODB_ENDPOINT, AWS_*, etc.
cargo run

cd harmoniis/frontend
npm install
npm run dev
```

**Add Harmonia endpoints** to backend (see Harmoniis API doc). Implement:
- `GET /api/v1/harmonia/trusted`
- `GET /api/v1/harmonia/challenge`
- `POST /api/v1/harmonia/register`
- `GET /api/v1/harmonia/auth/trusted`
- `POST /api/v1/harmonia/auth/trusted`

Frontend proxy: ensure `/api/harmonia/*` forwards to backend `/api/v1/harmonia/*`.

---

### Step 2: rumqttd + Auth Bridge

```bash
# Install rumqttd
cargo install rumqttd

# Run (no PGP initially — use for local dev without auth)
rumqttd
```

For genesis local testing, **PGP auth can be disabled** — allow all connections on localhost. Enable PGP auth when deploying.

---

### Step 3: Harmonia Core Tools

```bash
cd agent
cargo build --workspace
```

**Verify each tool:**
```bash
cargo test -p harmonia-vault
cargo test -p harmonia-memory
cargo test -p harmonia-mqtt-client
# ... etc
```

**Vault + OpenRouter test:**
1. Set `OPENROUTER_API_KEY` in env
2. Set `VAULT_MASTER_KEY` in env
3. Run vault init, register openrouter key
4. Call `vault_inject_request` with `:openrouter` — verify HTTP request succeeds

---

### Step 4: Harmonia Agent (SBCL)

```bash
cd agent/harmonia
# Bootstrap: install SBCL, Quicklisp, load CFFI
./scripts/bootstrap.sh

# Run agent (without Phoenix initially)
sbcl --load src/core/boot.lisp --eval '(harmonia:start)'
```

**Genesis config** (`config/agent.sexp`):
```lisp
(:evolution (:enabled nil))  ; Evolution OFF until closed loop passes
```

**Verify:**
1. All tools load
2. MQTT connects to rumqttd
3. OpenRouter backend initializes (or_init returns 0)
4. Agent responds to first prompt on `harmonia/+/cmd/+/prompt`

---

### Step 5: Model Selection (First Prompt vs Evolution)

| Task | Model | Rationale |
|------|-------|-----------|
| User prompt (routine) | kimi-k2.5 | Fast, cheap |
| User prompt (complex) | grok-code-fast-1 | Logic, refactor |
| User prompt (critical) | claude-opus-4.6 | Deep reasoning |
| Self-rewrite | claude-opus-4.6 | Only model for evolution |
| Code generation (Forge) | grok-code-fast-1 | Fast, good at code |
| Data generation (benchmarks) | kimi-k2.5 | Cheap |

Genesis must correctly route the first user prompt. Add a simple heuristic: short prompt → kimi, long/complex → grok, explicit "think deeply" → opus.

---

### Step 6: Closed Loop Test (Local)

**Prerequisites:**
- rumqttd running on localhost:1883
- Harmonia agent running (SBCL)
- MQTT client (e.g., `mosquitto_pub`) or a minimal test script

**Test script:**
```bash
# Publish a prompt to the agent
mosquitto_pub -h localhost -t "harmonia/agent-1/cmd/device-1/prompt" \
  -m '{"type":"prompt","text":"What is 2+2?"}'

# Subscribe to responses
mosquitto_sub -h localhost -t "harmonia/agent-1/response/#" -v
```

**Expected:** Agent receives prompt, calls OpenRouter (via vault-injected HTTP), returns response on `harmonia/agent-1/response/device-1/...`.

**Validation checklist:**
- [ ] Vault injects OpenRouter key (no key in Lisp)
- [ ] Model selection picks kimi for simple prompt
- [ ] Response arrives on MQTT
- [ ] No evolution occurs (evolution.enabled = false)

---

### Step 7: OS4-iOS + OS4-Android

Build both apps with Bazel. Point MQTT broker to local rumqttd for local testing.

**iOS:**
```bash
cd agent/OS4-iOS
bazel build //:OS4
# Run on simulator or device
```

**Android:**
```bash
cd agent/OS4-Android
bazel build //:OS4
adb install ...
```

**Local MQTT:** Set broker URL to `mqtt://localhost:1883` (or host machine IP for device).

---

### Step 8: Deploy

**TestFlight (iOS):**
```bash
./scripts/build_and_upload.sh 1.0.0
```

**Google Play (Android, internal track):**
```bash
./scripts/build_and_upload.sh 1.0.0
```

**Agent (pkgsrc):** After genesis is stable, build release tarball for NetBSD.

---

## 5. PGP Identity in Agent

The agent uses `lib/tools/pgp-identity` (optional tool) for its PGP operations. The agent can have multiple identities and label them:

```lisp
;; config/identities.sexp
(
  (:primary
   :fingerprint "ABC123..."
   :label "agent-primary"
   :use-for (:mqtt :signing))
  (:deploy
   :fingerprint "DEF456..."
   :label "agent-deploy-key"
   :use-for (:git :s3))
)
```

For genesis, a single identity is sufficient. The agent generates or loads a PGP keypair at first boot, registers it with harmoniis.com via `POST /api/harmonia/register`, and uses it for MQTT authentication.

---

## 6. File Layout Summary

```
agent/
├── harmonia/
│   ├── src/           # Lisp (genesis state)
│   ├── lib/           # Rust tools
│   ├── config/        # tools.sexp, model-policy.sexp, matrix-topology.sexp, parallel-policy.sexp, harmony-policy.sexp
│   └── scripts/       # bootstrap.sh, run.sh
├── OS4-iOS/
├── OS4-Android/
└── rumqttd.conf       # MQTT broker config (or in harmonia/)

harmoniis/
├── backend/           # Add /api/v1/harmonia/* endpoints
└── frontend/          # Proxy /api/harmonia/* → backend
```

---

## 7. References

- **rumqttd:** https://github.com/bytebeamio/rumqtt
- **PGP auth flow:** Harmoniis API doc §Harmonia
- **A2UI / MQTT protocol:** A2UI_SPEC.md
- **Vault / OpenRouter:** HARMONIA.md §Vault, §OpenRouter
- **CI/CD:** CICD.md
