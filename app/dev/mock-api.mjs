import { constants, generateKeyPairSync, privateDecrypt } from "node:crypto";
import { createServer } from "node:http";

const PORT = Number(process.env.MOCK_API_PORT ?? 8000);
const SETUP_CODE = "TEST1234";
const TOTP_CODE = "123456";
const TOTP_USERNAME = "totp";
const FINGERPRINT = Array.from({ length: 32 }, (_, index) =>
    ((index * 37 + 11) % 256).toString(16).padStart(2, "0").toUpperCase(),
).join(":");

const { publicKey, privateKey } = generateKeyPairSync("rsa", {
    modulusLength: 2048,
    publicKeyEncoding: { type: "pkcs1", format: "pem" },
    privateKeyEncoding: { type: "pkcs8", format: "pem" },
});

const GIB = 1024 ** 3;
const NODES = [
    { id: "n1", name: "fra-node-01", ip_address: "192.168.1.53", status: "online", role: "master", cpu: 4, memory: 14 },
    { id: "n2", name: "fra-node-02", ip_address: "192.168.1.54", status: "offline", role: "worker", cpu: null, memory: null },
];

let setupCompleted = process.env.MOCK_STATE === "ready";

function jitter(base) {
    return Math.max(1, base + (Math.random() - 0.5) * 6);
}

function agent(node) {
    return {
        id: node.id,
        name: node.name,
        hostname: node.name,
        ip_address: node.ip_address,
        os_type: "linux",
        os_version: "nixos",
        architecture: "x86_64",
        agent_version: "mock",
        status: node.status,
        last_heartbeat: null,
        registered_at: new Date().toISOString(),
        cordoned: false,
        maintenance_until: null,
        role: node.role,
    };
}

function nodeMetrics(node) {
    const online = node.status === "online";
    return {
        agent_id: node.id,
        hostname: node.name,
        status: node.status,
        cpu_usage_percent: online ? jitter(node.cpu) : null,
        memory_total_bytes: online ? 16 * GIB : null,
        memory_used_bytes: online ? (16 * GIB * node.memory) / 100 : null,
        disk_total_bytes: online ? 500 * GIB : null,
        disk_used_bytes: online ? 45 * GIB : null,
    };
}

function clusterStats() {
    return {
        node_count: NODES.length,
        online_count: NODES.filter((node) => node.status === "online").length,
        total_cpu_cores: 8,
        avg_cpu_usage_percent: jitter(4),
        total_memory_bytes: 16 * GIB,
        used_memory_bytes: 1.1 * GIB,
        total_disk_bytes: 500 * GIB,
        used_disk_bytes: 45 * GIB,
        nodes: NODES.map(nodeMetrics),
    };
}

function healthHistory() {
    const hour = 3_600_000;
    const start = Math.floor(Date.now() / hour) * hour - 24 * hour;
    return Array.from({ length: 25 }, (_, index) => ({
        bucket: new Date(start + index * hour).toISOString().replace("Z", ""),
        online_count: 1,
    }));
}

const AGENT_ROUTE = /^\/api\/agents\/([^/]+)(\/metrics\/latest|\/disks)?$/;

function latestMetrics(node) {
    const metrics = nodeMetrics(node);
    return {
        id: `m-${node.id}`,
        agent_id: node.id,
        timestamp: new Date().toISOString(),
        cpu_model: "Mock CPU",
        cpu_cores: 8,
        cpu_threads: 16,
        cpu_usage_percent: metrics.cpu_usage_percent ?? 0,
        memory_total_bytes: metrics.memory_total_bytes ?? 0,
        memory_used_bytes: metrics.memory_used_bytes ?? 0,
        memory_usage_percent: node.memory ?? 0,
        disk_total_bytes: metrics.disk_total_bytes ?? 0,
        disk_used_bytes: metrics.disk_used_bytes ?? 0,
        disk_usage_percent: 9,
        network_rx_bytes: 1.6 * GIB,
        network_tx_bytes: 1.5 * GIB,
        os_name: "NixOS",
        os_version: "25.11",
        kernel_version: "6.12.1",
        hostname: node.name,
        uptime_seconds: 240_000,
    };
}

const DISKS = [
    { device: "/dev/sda", model: "QEMU HARDDISK", media_type: "ssd", total_bytes: 120 * GIB, used_bytes: 6.9 * GIB, mount_point: "/", smart: null },
    { device: "/dev/sr0", model: "QEMU DVD-ROM", media_type: "optical", total_bytes: 1.8 * GIB, used_bytes: 0, mount_point: null, smart: null },
];

function agentRoute(method, path) {
    const match = method === "GET" ? AGENT_ROUTE.exec(path) : null;
    const node = match && NODES.find((entry) => entry.id === match[1]);
    if (!node) return null;
    const handlers = {
        undefined: (_body, response) => send(response, 200, agent(node)),
        "/metrics/latest": (_body, response) => send(response, 200, latestMetrics(node)),
        "/disks": (_body, response) => send(response, 200, DISKS),
    };
    return handlers[match[2]];
}

const USAGE_RANGES = { "1h": [60, 60_000], "24h": [96, 900_000], "7d": [168, 3_600_000] };

function usageHistory(range) {
    const [count, step] = USAGE_RANGES[range] ?? USAGE_RANGES["1h"];
    const end = Math.floor(Date.now() / step) * step;
    return Array.from({ length: count }, (_, index) => ({
        bucket: new Date(end - (count - 1 - index) * step).toISOString().replace("Z", ""),
        cpu: jitter(5 + Math.sin(index / 6) * 3),
        memory: jitter(14 + Math.sin(index / 9) * 2),
        disk: 9,
    }));
}

const EVENTS = [
    { id: "e1", agent_id: "n2", event_type: "node_offline", affected_workloads: null, duration_ms: null, created_at: new Date(Date.now() - 14 * 60_000).toISOString(), severity: "critical", status: "open", resolved_at: null, message: "fra-node-02 stopped responding to heartbeats" },
    { id: "e2", agent_id: "n1", event_type: "cluster_initialized", affected_workloads: null, duration_ms: null, created_at: new Date(Date.now() - 6 * 86_400_000).toISOString(), severity: "info", status: "resolved", resolved_at: null, message: "fra-node-01 set up as the first node" },
];

function decrypt(encoded) {
    return privateDecrypt(
        { key: privateKey, padding: constants.RSA_PKCS1_OAEP_PADDING, oaepHash: "sha256" },
        Buffer.from(encoded, "base64"),
    ).toString("utf8");
}

function session(username) {
    return {
        token: "mock-token",
        user_id: "00000000-0000-0000-0000-000000000001",
        username,
        two_factor_enabled: username === TOTP_USERNAME,
        force_password_change: false,
    };
}

function readBody(request) {
    return new Promise((resolve) => {
        const chunks = [];
        request.on("data", (chunk) => chunks.push(chunk));
        request.on("end", () => resolve(chunks.length ? JSON.parse(Buffer.concat(chunks)) : {}));
    });
}

function send(response, status, body) {
    response.writeHead(status, { "Content-Type": "application/json" });
    response.end(JSON.stringify(body));
}

function handleSetup(body, response) {
    if (setupCompleted) return send(response, 410, { error: "setup already completed" });
    if (body.setup_code !== SETUP_CODE) return send(response, 401, { error: "invalid setup code" });
    setupCompleted = true;
    send(response, 200, { ...session(body.admin_email), recovery_key: "ABCD-EFGH-JKLM-NPQR-STUV-WXYZ" });
}

function handleLogin(body, response) {
    const password = decrypt(body.encrypted_password);
    const needsCode = body.username === TOTP_USERNAME && body.two_factor_code !== TOTP_CODE;
    if (needsCode && body.two_factor_code === null) return send(response, 403, { error: "two factor required" });
    if (needsCode || password === "wrong") return send(response, 401, { error: "invalid credentials" });
    send(response, 200, session(body.username));
}

const routes = {
    "GET /api/public-key": (_body, response) => send(response, 200, { public_key: publicKey }),
    "GET /api/setup/status": (_body, response) =>
        send(response, 200, {
            state: setupCompleted ? "ready" : "setup_required",
            version: "mock",
            ca_fingerprint: FINGERPRINT,
        }),
    "GET /api/setup/ca.crt": (_body, response) => {
        response.writeHead(200, { "Content-Type": "application/x-pem-file" });
        response.end("-----BEGIN CERTIFICATE-----\nmock\n-----END CERTIFICATE-----\n");
    },
    "POST /api/setup": handleSetup,
    "POST /api/setup/join": (_body, response) => send(response, 200, {}),
    "GET /api/auth/providers": (_body, response) =>
        send(response, 200, [{ slug: "github", display_name: "GitHub" }]),
    "POST /api/login": handleLogin,
    "GET /api/validate-session": (_body, response) =>
        send(response, 200, {
            id: "00000000-0000-0000-0000-000000000001",
            username: "admin",
            email: "admin@local.com",
            gravatar_email: null,
            two_factor_enabled: false,
            force_password_change: false,
        }),
    "GET /api/settings/avatar-fallback": (_body, response) => send(response, 200, { style: "initials" }),
    "GET /api/system/update/status": (_body, response) => send(response, 200, { current_version: "0.2.2" }),
    "GET /api/system/releases": (_body, response) =>
        send(response, 200, { current_version: "0.2.2", update_available: true, latest_stable: "v0.3.0", releases: [] }),
    "GET /api/agents": (_body, response) => send(response, 200, NODES.map(agent)),
    "GET /api/system/stats": (_body, response) => send(response, 200, clusterStats()),
    "GET /api/system/stats/history": (_body, response) => send(response, 200, healthHistory()),
    "GET /api/system/stats/usage": (_body, response, query) => send(response, 200, usageHistory(query.get("range"))),
    "GET /api/events": (_body, response) => send(response, 200, EVENTS),
    "GET /api/workloads": (_body, response) => send(response, 200, []),
    "GET /api/volumes": (_body, response) => send(response, 200, []),
    "GET /api/buckets": (_body, response) => send(response, 200, []),
    "POST /__reset": (_body, response) => {
        setupCompleted = false;
        send(response, 200, { state: "setup_required" });
    },
};

createServer(async (request, response) => {
    const path = request.url.split("?")[0];
    const route = routes[`${request.method} ${path}`] ?? agentRoute(request.method, path);
    if (!route) return send(response, 404, { error: "not found" });
    route(await readBody(request), response, new URL(request.url, "http://localhost").searchParams);
}).listen(PORT, () => {
    console.info(`[INFO] mock api listening port=${PORT} setup_code=${SETUP_CODE} totp_user=${TOTP_USERNAME} totp_code=${TOTP_CODE}`);
});
