#!/bin/bash

# CSF Registry Service Test Script

set -e

REGISTRY_URL="http://localhost:8001"

echo "🧪 Testing CSF Registry Service"
echo "================================"
echo ""

# 1. Health Check
echo "1️⃣  Health Check..."
curl -f "${REGISTRY_URL}/health" || { echo "❌ Health check failed"; exit 1; }
echo "✅ Health check passed"
echo ""

# 2. Create Registration Token
echo "2️⃣  Creating registration token..."
TOKEN_RESPONSE=$(curl -s -X POST "${REGISTRY_URL}/admin/tokens" \
  -H "Content-Type: application/json" \
  -d '{
    "description": "Test Token",
    "created_by": "test-admin",
    "ttl_hours": 24
  }')

echo "Response: ${TOKEN_RESPONSE}"
TOKEN=$(echo ${TOKEN_RESPONSE} | jq -r '.token')
echo "✅ Token created: ${TOKEN}"
echo ""

# 3. List Tokens
echo "3️⃣  Listing all tokens..."
curl -s "${REGISTRY_URL}/admin/tokens" | jq '.'
echo "✅ Tokens listed"
echo ""

# 4. Register Agent
echo "4️⃣  Registering agent..."
REGISTER_RESPONSE=$(curl -s -X POST "${REGISTRY_URL}/agents/register" \
  -H "Content-Type: application/json" \
  -d "{
    \"registration_token\": \"${TOKEN}\",
    \"name\": \"test-agent-1\",
    \"hostname\": \"test-host-1\",
    \"os_type\": \"linux\",
    \"os_version\": \"Ubuntu 22.04\",
    \"architecture\": \"x86_64\",
    \"agent_version\": \"1.0.0\",
    \"tags\": {
      \"environment\": \"test\",
      \"region\": \"eu-west-1\"
    }
  }")

echo "Response: ${REGISTER_RESPONSE}"
AGENT_ID=$(echo ${REGISTER_RESPONSE} | jq -r '.agent_id')
API_KEY=$(echo ${REGISTER_RESPONSE} | jq -r '.api_key')
echo "✅ Agent registered"
echo "   Agent ID: ${AGENT_ID}"
echo "   API Key: ${API_KEY}"
echo ""

# 5. Send Heartbeat
echo "5️⃣  Sending heartbeat..."
HEARTBEAT_RESPONSE=$(curl -s -X POST "${REGISTRY_URL}/agents/${AGENT_ID}/heartbeat" \
  -H "Content-Type: application/json" \
  -H "X-API-Key: ${API_KEY}" \
  -d '{
    "status": "online"
  }')

echo "Response: ${HEARTBEAT_RESPONSE}"
echo "✅ Heartbeat sent"
echo ""

# 6. List Agents
echo "6️⃣  Listing all agents..."
curl -s "${REGISTRY_URL}/admin/agents" | jq '.'
echo "✅ Agents listed"
echo ""

# 7. Get Agent Details
echo "7️⃣  Getting agent details..."
curl -s "${REGISTRY_URL}/admin/agents/${AGENT_ID}" | jq '.'
echo "✅ Agent details retrieved"
echo ""

# 8. Get Statistics
echo "8️⃣  Getting statistics..."
curl -s "${REGISTRY_URL}/admin/statistics" | jq '.'
echo "✅ Statistics retrieved"
echo ""

# 9. Try to reuse token (should fail)
echo "9️⃣  Testing token reuse (should fail)..."
REUSE_RESPONSE=$(curl -s -X POST "${REGISTRY_URL}/agents/register" \
  -H "Content-Type: application/json" \
  -d "{
    \"registration_token\": \"${TOKEN}\",
    \"name\": \"test-agent-2\",
    \"hostname\": \"test-host-2\",
    \"os_type\": \"linux\",
    \"os_version\": \"Ubuntu 22.04\",
    \"architecture\": \"x86_64\",
    \"agent_version\": \"1.0.0\"
  }")

if echo "${REUSE_RESPONSE}" | grep -q "error"; then
  echo "✅ Token reuse correctly rejected"
  echo "   Error: $(echo ${REUSE_RESPONSE} | jq -r '.error')"
else
  echo "❌ Token reuse should have failed!"
  exit 1
fi
echo ""

# 10. Test invalid API key (should fail)
echo "🔟  Testing invalid API key (should fail)..."
INVALID_RESPONSE=$(curl -s -X POST "${REGISTRY_URL}/agents/${AGENT_ID}/heartbeat" \
  -H "Content-Type: application/json" \
  -H "X-API-Key: invalid_key_xyz" \
  -d '{
    "status": "online"
  }')

if echo "${INVALID_RESPONSE}" | grep -q "error"; then
  echo "✅ Invalid API key correctly rejected"
  echo "   Error: $(echo ${INVALID_RESPONSE} | jq -r '.error')"
else
  echo "❌ Invalid API key should have been rejected!"
  exit 1
fi
echo ""

# 11. Deregister Agent
echo "1️⃣1️⃣  Deregistering agent..."
curl -s -X POST "${REGISTRY_URL}/admin/agents/${AGENT_ID}" 
echo "✅ Agent deregistered"
echo ""

echo "================================"
echo "✅ All tests passed!"
echo "================================"
