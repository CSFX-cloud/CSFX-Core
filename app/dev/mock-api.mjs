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

let setupCompleted = process.env.MOCK_STATE === "ready";

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
    "POST /__reset": (_body, response) => {
        setupCompleted = false;
        send(response, 200, { state: "setup_required" });
    },
};

createServer(async (request, response) => {
    const route = routes[`${request.method} ${request.url.split("?")[0]}`];
    if (!route) return send(response, 404, { error: "not found" });
    route(await readBody(request), response);
}).listen(PORT, () => {
    console.info(`[INFO] mock api listening port=${PORT} setup_code=${SETUP_CODE} totp_user=${TOTP_USERNAME} totp_code=${TOTP_CODE}`);
});
