#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/7] build mqtt client dylib"
cargo build --release -p harmonia-mqtt-client

WORKDIR="$(mktemp -d /tmp/harmonia-mqtt-tls-XXXXXX)"
trap 'if [ -n "${MOSQ_PID:-}" ]; then kill "$MOSQ_PID" >/dev/null 2>&1 || true; fi' EXIT
echo "WORKDIR=$WORKDIR"

echo "[2/7] resolve local PGP signing identity"
FPR="${HARMONIA_PGP_FPR:-$(gpg --list-secret-keys --with-colons | awk -F: '/^fpr:/ {print $10; exit}')}"
if [ -z "$FPR" ]; then
  echo "No local PGP secret key found."
  exit 1
fi
gpg --armor --export "$FPR" > "$WORKDIR/pgp_pub.asc"

echo "[3/7] generate local TLS CA/server/client cert chain"
openssl req -x509 -newkey rsa:2048 -nodes -days 1 \
  -subj "/CN=Harmonia Local CA" \
  -keyout "$WORKDIR/ca.key" -out "$WORKDIR/ca.crt" >/dev/null 2>&1

openssl req -newkey rsa:2048 -nodes \
  -subj "/CN=localhost" \
  -keyout "$WORKDIR/server.key" -out "$WORKDIR/server.csr" >/dev/null 2>&1
cat > "$WORKDIR/server.ext" <<EOF
subjectAltName=DNS:localhost,IP:127.0.0.1
extendedKeyUsage=serverAuth
EOF
openssl x509 -req -days 1 -in "$WORKDIR/server.csr" \
  -CA "$WORKDIR/ca.crt" -CAkey "$WORKDIR/ca.key" -CAcreateserial \
  -out "$WORKDIR/server.crt" -extfile "$WORKDIR/server.ext" >/dev/null 2>&1

openssl req -newkey rsa:2048 -nodes \
  -subj "/CN=harmonia-client" \
  -keyout "$WORKDIR/client.key" -out "$WORKDIR/client.csr" >/dev/null 2>&1
cat > "$WORKDIR/client.ext" <<EOF
extendedKeyUsage=clientAuth
EOF
openssl x509 -req -days 1 -in "$WORKDIR/client.csr" \
  -CA "$WORKDIR/ca.crt" -CAkey "$WORKDIR/ca.key" -CAcreateserial \
  -out "$WORKDIR/client.crt" -extfile "$WORKDIR/client.ext" >/dev/null 2>&1

echo "[4/7] bind PGP trust to TLS artifacts (sign + verify cert fingerprints)"
openssl x509 -in "$WORKDIR/ca.crt" -noout -fingerprint -sha256 > "$WORKDIR/ca.fpr.txt"
openssl x509 -in "$WORKDIR/client.crt" -noout -fingerprint -sha256 > "$WORKDIR/client.fpr.txt"
gpg --batch --yes --armor --detach-sign -u "$FPR" -o "$WORKDIR/ca.fpr.asc" "$WORKDIR/ca.fpr.txt"
gpg --batch --yes --armor --detach-sign -u "$FPR" -o "$WORKDIR/client.fpr.asc" "$WORKDIR/client.fpr.txt"
gpg --verify "$WORKDIR/ca.fpr.asc" "$WORKDIR/ca.fpr.txt" >/dev/null 2>&1
gpg --verify "$WORKDIR/client.fpr.asc" "$WORKDIR/client.fpr.txt" >/dev/null 2>&1

echo "[5/7] start local mTLS mosquitto broker"
cat > "$WORKDIR/mosquitto.conf" <<EOF
listener 8883 localhost
persistence false
allow_anonymous true
cafile $WORKDIR/ca.crt
certfile $WORKDIR/server.crt
keyfile $WORKDIR/server.key
require_certificate true
use_identity_as_username true
EOF

/usr/local/opt/mosquitto/sbin/mosquitto -c "$WORKDIR/mosquitto.conf" -v > "$WORKDIR/mosquitto.log" 2>&1 &
MOSQ_PID=$!
sleep 1

echo "[6/7] run Harmonia mqtt-client against local mTLS broker"
TOPIC="harmonia/localtls/$(date +%s)-$$"
PAYLOAD="(event . tls-pgp-ok)"
HARMONIA_MQTT_BROKER="localhost:8883" \
HARMONIA_MQTT_TLS=1 \
HARMONIA_MQTT_CA_CERT="$WORKDIR/ca.crt" \
HARMONIA_MQTT_CLIENT_CERT="$WORKDIR/client.crt" \
HARMONIA_MQTT_CLIENT_KEY="$WORKDIR/client.key" \
sbcl --disable-debugger \
  --eval '(load #P"~/quicklisp/setup.lisp")' \
  --eval '(funcall (find-symbol "QUICKLOAD" (find-package :ql)) :cffi)' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_mqtt_client.dylib")' \
  --eval '(cffi:defcfun ("harmonia_mqtt_client_publish" mpub) :int (topic :string) (payload :string))' \
  --eval '(cffi:defcfun ("harmonia_mqtt_client_poll" mpoll) :pointer (topic :string))' \
  --eval '(cffi:defcfun ("harmonia_mqtt_client_free_string" mfree) :void (ptr :pointer))' \
  --eval '(cffi:defcfun ("harmonia_mqtt_client_last_error" merr) :pointer)' \
  --eval '(cffi:defcfun ("harmonia_mqtt_client_free_string" mfree2) :void (ptr :pointer))' \
  --eval "(format t \"~&MQTT_TLS_PUB_RC=~D~%\" (mpub \"$TOPIC\" \"$PAYLOAD\"))" \
  --eval '(let ((e (merr))) (unless (cffi:null-pointer-p e) (format t "~&MQTT_TLS_ERR1=~A~%" (cffi:foreign-string-to-lisp e)) (mfree2 e)))' \
  --eval "(let ((p (mpoll \"$TOPIC\"))) (if (cffi:null-pointer-p p) (progn (format t \"~&MQTT_TLS_POLL=NULL~%\") (let ((e (merr))) (unless (cffi:null-pointer-p e) (format t \"~&MQTT_TLS_ERR2=~A~%\" (cffi:foreign-string-to-lisp e)) (mfree2 e))) (sb-ext:exit :code 2)) (progn (format t \"~&MQTT_TLS_POLL=~A~%\" (cffi:foreign-string-to-lisp p)) (mfree p))))" \
  --quit

echo "[7/7] done"
echo "Local PGP+TLS MQTT validation complete."
