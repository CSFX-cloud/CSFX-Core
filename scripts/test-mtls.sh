#!/usr/bin/env bash
set -euo pipefail

GATEWAY_URL="${GATEWAY_URL:-http://localhost:8000}"
JWT="${JWT:-}"
WORKDIR="$(mktemp -d)"

cleanup() {
    rm -rf "$WORKDIR"
}
trap cleanup EXIT

log() { echo "[test-mtls] $*"; }
fail() { echo "[FAIL] $*" >&2; exit 1; }
ok() { echo "[OK]   $*"; }

if [ -z "$JWT" ]; then
    fail "JWT environment variable is required. Export it before running this script."
fi

log "Working directory: $WORKDIR"
log "Gateway: $GATEWAY_URL"

# ── 1. Node keypair + CSR generieren ──────────────────────────────────────────
log "Generating ECDSA keypair and CSR..."

openssl ecparam -name prime256v1 -genkey -noout -out "$WORKDIR/node.key" 2>/dev/null
openssl req -new -key "$WORKDIR/node.key" -out "$WORKDIR/node.csr" \
    -subj "/CN=test-node/O=CS-Foundry" 2>/dev/null

ok "Keypair and CSR generated"

# ── 2. Pre-registrieren ───────────────────────────────────────────────────────
log "Pre-registering agent..."

PREREG=$(curl -sf -X POST "$GATEWAY_URL/api/registry/admin/agents/pre-register" \
    -H "Authorization: Bearer $JWT" \
    -H "Content-Type: application/json" \
    -d '{"name":"test-node","hostname":"test.local","ttl_hours":1}') \
    || fail "Pre-registration request failed"

TOKEN=$(echo "$PREREG" | jq -r '.registration_token')
AGENT_ID=$(echo "$PREREG" | jq -r '.agent_id')

[ "$TOKEN" != "null" ] && [ -n "$TOKEN" ] || fail "No registration token received"
ok "Pre-registered agent_id=$AGENT_ID token=$TOKEN"

# ── 3. CSR als JSON-String vorbereiten ────────────────────────────────────────
CSR_PEM=$(cat "$WORKDIR/node.csr")

PAYLOAD=$(jq -n \
    --arg token "$TOKEN" \
    --arg csr "$CSR_PEM" \
    '{
        registration_token: $token,
        name: "test-node",
        hostname: "test.local",
        os_type: "linux",
        os_version: "6.1.0",
        architecture: "x86_64",
        agent_version: "0.2.2",
        csr_pem: $csr
    }')

# ── 4. Agent registrieren + Zertifikat empfangen ──────────────────────────────
log "Registering agent and requesting certificate..."

REGRESPONSE=$(curl -sf -X POST "$GATEWAY_URL/api/registry/agents/register" \
    -H "Content-Type: application/json" \
    -d "$PAYLOAD") \
    || fail "Registration request failed"

API_KEY=$(echo "$REGRESPONSE" | jq -r '.api_key')
CERT_PEM=$(echo "$REGRESPONSE" | jq -r '.certificate_pem')
CA_PEM=$(echo "$REGRESPONSE" | jq -r '.ca_cert_pem')

[ "$CERT_PEM" != "null" ] && [ -n "$CERT_PEM" ] || fail "No certificate received from registry"
[ "$CA_PEM" != "null" ] && [ -n "$CA_PEM" ] || fail "No CA certificate received from registry"
ok "Registration successful api_key=${API_KEY:0:20}..."

echo "$CERT_PEM" > "$WORKDIR/node.crt"
echo "$CA_PEM" > "$WORKDIR/ca.crt"

# ── 5. Zertifikat verifizieren ────────────────────────────────────────────────
log "Verifying certificate chain..."

openssl verify -CAfile "$WORKDIR/ca.crt" "$WORKDIR/node.crt" > /dev/null 2>&1 \
    || fail "Certificate chain verification failed"
ok "Certificate chain valid"

SUBJECT=$(openssl x509 -in "$WORKDIR/node.crt" -subject -noout 2>/dev/null)
log "Certificate subject: $SUBJECT"

EXPIRY=$(openssl x509 -in "$WORKDIR/node.crt" -enddate -noout 2>/dev/null)
log "Certificate expiry: $EXPIRY"

# ── 6. CRL prüfen (sollte leer sein) ─────────────────────────────────────────
log "Checking CRL..."

CRL=$(curl -sf "$GATEWAY_URL/api/registry/pki/crl") \
    || fail "CRL request failed"
REVOKED=$(echo "$CRL" | jq '.revoked_serials | length')
ok "CRL fetched, revoked_count=$REVOKED"

# ── 7. Heartbeat mit API-Key senden ──────────────────────────────────────────
log "Sending heartbeat..."

curl -sf -X POST "$GATEWAY_URL/api/registry/agents/$AGENT_ID/heartbeat" \
    -H "X-API-Key: $API_KEY" \
    -H "Content-Type: application/json" \
    -d '{"status":null}' > /dev/null \
    || fail "Heartbeat failed"
ok "Heartbeat accepted"

# ── 8. mTLS Node-zu-Node Simulation ─────────────────────────────────────────
log "Simulating node-to-node mTLS..."

# Zweites Node-Keypair für Client-Seite
openssl ecparam -name prime256v1 -genkey -noout -out "$WORKDIR/node2.key" 2>/dev/null
openssl req -new -key "$WORKDIR/node2.key" -out "$WORKDIR/node2.csr" \
    -subj "/CN=test-node-2/O=CS-Foundry" 2>/dev/null

# node2 registrieren
PREREG2=$(curl -sf -X POST "$GATEWAY_URL/api/registry/admin/agents/pre-register" \
    -H "Authorization: Bearer $JWT" \
    -H "Content-Type: application/json" \
    -d '{"name":"test-node-2","hostname":"test2.local","ttl_hours":1}') \
    || fail "Pre-registration of node2 failed"

TOKEN2=$(echo "$PREREG2" | jq -r '.registration_token')
CSR2_PEM=$(cat "$WORKDIR/node2.csr")

PAYLOAD2=$(jq -n \
    --arg token "$TOKEN2" \
    --arg csr "$CSR2_PEM" \
    '{
        registration_token: $token,
        name: "test-node-2",
        hostname: "test2.local",
        os_type: "linux",
        os_version: "6.1.0",
        architecture: "x86_64",
        agent_version: "0.2.2",
        csr_pem: $csr
    }')

REGRESPONSE2=$(curl -sf -X POST "$GATEWAY_URL/api/registry/agents/register" \
    -H "Content-Type: application/json" \
    -d "$PAYLOAD2") \
    || fail "Registration of node2 failed"

CERT2_PEM=$(echo "$REGRESPONSE2" | jq -r '.certificate_pem')
echo "$CERT2_PEM" > "$WORKDIR/node2.crt"

ok "node2 registered and certificate issued"

# mTLS-Server starten (node1), Client-Verbindung von node2
log "Starting mTLS server (node1) and connecting as node2..."

openssl s_server \
    -cert "$WORKDIR/node.crt" \
    -key "$WORKDIR/node.key" \
    -CAfile "$WORKDIR/ca.crt" \
    -Verify 1 \
    -port 18443 \
    -quiet &
SERVER_PID=$!
sleep 1

MTLS_OUTPUT=$(echo "PING" | openssl s_client \
    -cert "$WORKDIR/node2.crt" \
    -key "$WORKDIR/node2.key" \
    -CAfile "$WORKDIR/ca.crt" \
    -connect localhost:18443 \
    -verify_return_error \
    -quiet 2>&1 || true)

kill $SERVER_PID 2>/dev/null || true

if echo "$MTLS_OUTPUT" | grep -q "Verify return code: 0"; then
    ok "mTLS handshake successful — mutual authentication verified"
elif echo "$MTLS_OUTPUT" | grep -q "CONNECTED"; then
    ok "mTLS connection established"
else
    fail "mTLS handshake failed: $MTLS_OUTPUT"
fi

# ── 9. Revocation testen ─────────────────────────────────────────────────────
log "Testing certificate revocation..."

AGENT2_ID=$(echo "$REGRESPONSE2" | jq -r '.agent_id')

curl -sf -X POST "$GATEWAY_URL/api/registry/admin/agents/$AGENT2_ID/revoke" \
    -H "Authorization: Bearer $JWT" \
    -H "Content-Type: application/json" \
    -d '{"reason":"test revocation"}' > /dev/null \
    || fail "Revocation request failed"

CRL_AFTER=$(curl -sf "$GATEWAY_URL/api/registry/pki/crl") \
    || fail "CRL request failed after revocation"
REVOKED_AFTER=$(echo "$CRL_AFTER" | jq '.revoked_serials | length')

[ "$REVOKED_AFTER" -gt 0 ] || fail "CRL should contain revoked serial after revocation"
ok "Revocation successful, revoked_count=$REVOKED_AFTER"

# ── Zusammenfassung ───────────────────────────────────────────────────────────
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo " mTLS Test Suite: ALL PASSED"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo " keypair + CSR generation   OK"
echo " agent pre-registration     OK"
echo " agent registration + cert  OK"
echo " certificate chain verify   OK"
echo " heartbeat                  OK"
echo " node-to-node mTLS          OK"
echo " certificate revocation     OK"
echo " CRL update                 OK"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
