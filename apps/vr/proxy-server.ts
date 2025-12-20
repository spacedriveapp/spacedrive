#!/usr/bin/env bun
/**
 * WebSocket-to-TCP Proxy Server for Spacedrive VR
 *
 * Bridges WebSocket connections from VR headset to Spacedrive daemon's TCP socket.
 * Run this on your laptop alongside the daemon.
 *
 * Uses self-signed certificate for HTTPS/WSS (required for secure WebSocket from HTTPS page)
 */

import { serve } from "bun";
import { Socket } from "net";
import { resolve, join } from "path";
import { existsSync, writeFileSync } from "fs";
import { execSync } from "child_process";
import { homedir } from "os";

const DAEMON_HOST = "127.0.0.1";
const DAEMON_PORT = 6969;
const DAEMON_HTTP_PORT = 9420; // HTTP server for sidecars (not used currently)
const PROXY_PORT = 8080;

// Spacedrive libraries directory (macOS default)
const LIBRARIES_DIR = join(
	homedir(),
	"Library",
	"Application Support",
	"spacedrive",
	"libraries",
);

// Cache library paths from daemon
const libraryPathsCache = new Map<string, string>();

/**
 * Query daemon for library path
 */
async function getLibraryPath(libraryId: string): Promise<string | null> {
	return new Promise((resolve, reject) => {
		const socket = new Socket();

		socket.connect(DAEMON_PORT, DAEMON_HOST, () => {
			// Send libraries.list query
			const query = JSON.stringify({
				Query: {
					method: "query:libraries.list",
					library_id: null,
					payload: { include_stats: false },
				},
			});
			socket.write(query + "\n");
		});

		socket.on("data", (data) => {
			try {
				const response = JSON.parse(data.toString());
				if (response.JsonOk) {
					const libraries = response.JsonOk;
					const library = libraries.find(
						(lib: any) => lib.id === libraryId,
					);
					resolve(library?.path || null);
				} else {
					resolve(null);
				}
			} catch (err) {
				console.error("Failed to parse daemon response:", err);
				resolve(null);
			}
			socket.destroy();
		});

		socket.on("error", (err) => {
			console.error("Failed to query daemon:", err);
			resolve(null);
			socket.destroy();
		});

		setTimeout(() => {
			socket.destroy();
			reject(new Error("Library query timeout"));
		}, 5000);
	});
}

// Generate self-signed certificate if it doesn't exist
const certDir = resolve(import.meta.dir, ".certs");
const certFile = resolve(certDir, "cert.pem");
const keyFile = resolve(certDir, "key.pem");

if (!existsSync(certFile) || !existsSync(keyFile)) {
	console.log("📜 Generating self-signed certificate...");

	try {
		execSync(`mkdir -p ${certDir}`);

		// Create OpenSSL config for SAN (Subject Alternative Names)
		// This allows the cert to work for localhost AND IP addresses
		const configFile = resolve(certDir, "openssl.cnf");
		const configContent = `
[req]
default_bits = 2048
prompt = no
default_md = sha256
distinguished_name = dn
req_extensions = v3_req

[dn]
CN = localhost

[v3_req]
keyUsage = keyEncipherment, dataEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
DNS.1 = localhost
DNS.2 = *.local
IP.1 = 127.0.0.1
IP.2 = 0.0.0.0
`;
		writeFileSync(configFile, configContent);

		execSync(
			`openssl req -x509 -newkey rsa:2048 -nodes -sha256 ` +
				`-keyout ${keyFile} -out ${certFile} -days 365 -config ${configFile} -extensions v3_req`,
			{ stdio: "ignore" },
		);
		console.log("✅ Certificate generated with SAN for localhost and IPs");
	} catch (err) {
		console.error(
			"❌ Failed to generate certificate. Install OpenSSL or provide cert manually.",
		);
		console.error("   Run: brew install openssl  (macOS)");
		console.error("   Error:", err);
		process.exit(1);
	}
}

console.log(`🚀 Starting Spacedrive VR Proxy Server...`);
console.log(`📡 Daemon RPC: tcp://${DAEMON_HOST}:${DAEMON_PORT}`);
console.log(`📁 Libraries: ${LIBRARIES_DIR}`);
console.log(`🌐 Listening: https://0.0.0.0:${PROXY_PORT} (self-signed cert)`);

serve({
	port: PROXY_PORT,

	// Enable TLS with self-signed certificate for development
	// This allows wss:// connections from HTTPS pages
	tls: {
		cert: Bun.file(certFile),
		key: Bun.file(keyFile),
	},

	async fetch(req, server) {
		const url = new URL(req.url);

		// WebSocket upgrade for daemon communication
		if (url.pathname === "/ws" && server.upgrade(req)) {
			return; // WebSocket connection handled below
		}

		// Serve sidecar files directly from filesystem
		// Format: /sidecar/{libraryId}/{contentUuid}/{kind}/{variant}.{format}
		if (url.pathname.startsWith("/sidecar/")) {
			try {
				const parts = url.pathname.split("/").filter(Boolean);
				if (parts.length !== 5) {
					throw new Error("Invalid sidecar path format");
				}

				const [_, libraryId, contentUuid, kind, variantWithExt] = parts;
				console.log(`📷 Serving sidecar: ${url.pathname}`);

				// Get library path (either from cache or query daemon)
				let libraryPath = libraryPathsCache.get(libraryId);
				if (!libraryPath) {
					// Query daemon for library info
					libraryPath = await getLibraryPath(libraryId);
					if (!libraryPath) {
						throw new Error(`Library ${libraryId} not found`);
					}
					libraryPathsCache.set(libraryId, libraryPath);
				}

				// Compute shard directories (h0, h1) from content UUID
				// Remove hyphens and take first 4 chars in pairs
				const hex = contentUuid.replace(/-/g, "").toLowerCase();
				const h0 = hex.substring(0, 2);
				const h1 = hex.substring(2, 4);

				// Build full path: {library}/sidecars/content/{h0}/{h1}/{uuid}/{kind}/{variant}.{ext}
				const sidecarPath = join(
					libraryPath,
					"sidecars",
					"content",
					h0,
					h1,
					contentUuid,
					kind,
					variantWithExt,
				);

				console.log(`   → Local path: ${sidecarPath}`);

				// Check if file exists
				if (!existsSync(sidecarPath)) {
					console.log(`   ❌ File not found`);
					return new Response("Sidecar not found", {
						status: 404,
						headers: { "Access-Control-Allow-Origin": "*" },
					});
				}

				// Serve the file
				const file = Bun.file(sidecarPath);
				return new Response(file, {
					headers: {
						"Access-Control-Allow-Origin": "*",
						"Content-Type": file.type || "application/octet-stream",
						"Cache-Control": "public, max-age=31536000", // Cache for 1 year
					},
				});
			} catch (error) {
				console.error("❌ Sidecar serve error:", error);
				return new Response("Sidecar error", {
					status: 500,
					headers: { "Access-Control-Allow-Origin": "*" },
				});
			}
		}

		// Health check endpoint
		if (url.pathname === "/health") {
			return new Response(
				JSON.stringify({
					status: "ok",
					daemon_tcp: `${DAEMON_HOST}:${DAEMON_PORT}`,
					daemon_http: `${DAEMON_HOST}:${DAEMON_HTTP_PORT}`,
					timestamp: new Date().toISOString(),
					message: "Proxy server is running",
				}),
				{
					headers: {
						"Content-Type": "application/json",
						"Access-Control-Allow-Origin": "*",
					},
				},
			);
		}

		// Test page to verify HTTPS and accept certificate
		if (url.pathname === "/" || url.pathname === "/test") {
			return new Response(
				`<!DOCTYPE html>
<html>
<head>
	<title>Spacedrive VR Proxy Test</title>
	<style>
		body {
			font-family: system-ui;
			max-width: 800px;
			margin: 50px auto;
			padding: 20px;
			background: #1a1a2e;
			color: #fff;
		}
		.status {
			padding: 20px;
			margin: 20px 0;
			border-radius: 8px;
			font-size: 18px;
		}
		.ok { background: #22c55e; }
		.error { background: #ef4444; }
		.info { background: #3b82f6; }
		button {
			background: #6366f1;
			color: white;
			border: none;
			padding: 15px 30px;
			font-size: 16px;
			border-radius: 8px;
			cursor: pointer;
			margin: 10px 0;
		}
		button:hover { background: #4f46e5; }
		#log {
			background: #0a0a0a;
			padding: 15px;
			border-radius: 8px;
			font-family: monospace;
			font-size: 14px;
			max-height: 400px;
			overflow-y: auto;
			margin-top: 20px;
		}
		.log-entry { margin: 5px 0; }
	</style>
</head>
<body>
	<h1>🚀 Spacedrive VR Proxy Server</h1>
	<div class="status ok">✅ Certificate accepted! HTTPS is working.</div>
	<div class="status info">
		<strong>Proxy Server:</strong> ${url.origin}<br>
		<strong>Daemon:</strong> ${DAEMON_HOST}:${DAEMON_PORT}<br>
		<strong>WebSocket URL:</strong> ${url.origin.replace("https", "wss")}/ws
	</div>

	<h2>Test WebSocket Connection</h2>
	<button onclick="testWebSocket()">Test WebSocket</button>
	<button onclick="clearLog()">Clear Log</button>

	<div id="log"></div>

	<script>
		const log = document.getElementById('log');

		function addLog(message, type = 'info') {
			const entry = document.createElement('div');
			entry.className = 'log-entry';
			entry.style.color = type === 'error' ? '#ef4444' : type === 'success' ? '#22c55e' : '#94a3b8';
			entry.textContent = new Date().toLocaleTimeString() + ' - ' + message;
			log.appendChild(entry);
			log.scrollTop = log.scrollHeight;
		}

		function clearLog() {
			log.innerHTML = '';
		}

		function testWebSocket() {
			const wsUrl = '${url.origin.replace("https", "wss")}/ws';
			addLog('Connecting to ' + wsUrl + '...');

			try {
				const ws = new WebSocket(wsUrl);

				ws.onopen = () => {
					addLog('✅ WebSocket connected!', 'success');

					// Send a ping to test daemon connection
					const ping = JSON.stringify({ "Ping": null });
					addLog('Sending ping: ' + ping);
					ws.send(ping);
				};

				ws.onmessage = (event) => {
					addLog('📨 Received: ' + event.data, 'success');
				};

				ws.onerror = (error) => {
					addLog('❌ WebSocket error: ' + error.type, 'error');
				};

				ws.onclose = (event) => {
					addLog('🔌 WebSocket closed: ' + event.code + ' - ' + event.reason);
				};

				// Close after 5 seconds
				setTimeout(() => {
					if (ws.readyState === WebSocket.OPEN) {
						addLog('Closing connection...');
						ws.close();
					}
				}, 5000);

			} catch (err) {
				addLog('❌ Failed to create WebSocket: ' + err.message, 'error');
			}
		}

		// Auto-test on load
		addLog('Page loaded successfully');
		addLog('HTTPS certificate is working');
	</script>
</body>
</html>`,
				{
					headers: {
						"Content-Type": "text/html",
						"Access-Control-Allow-Origin": "*",
					},
				},
			);
		}

		// CORS preflight
		if (req.method === "OPTIONS") {
			return new Response(null, {
				headers: {
					"Access-Control-Allow-Origin": "*",
					"Access-Control-Allow-Methods": "GET, POST, OPTIONS",
					"Access-Control-Allow-Headers": "Content-Type",
				},
			});
		}

		return new Response(
			"Spacedrive VR Proxy Server. Use /ws for WebSocket connection.",
			{
				headers: { "Access-Control-Allow-Origin": "*" },
			},
		);
	},

	websocket: {
		open(ws) {
			const connTime = new Date().toISOString();
			console.log(`\n✅ [${connTime}] WebSocket client connected`);
			// No persistent TCP connection - create one per request instead
		},

		message(ws, message) {
			const messageStr =
				typeof message === "string" ? message : message.toString();

			// Better logging: parse and show request type
			try {
				const parsed = JSON.parse(messageStr);
				const requestType = parsed.Query
					? "Query"
					: parsed.Action
						? "Action"
						: parsed.Ping !== undefined
							? "Ping"
							: parsed.Subscribe
								? "Subscribe"
								: "Unknown";
				const method =
					parsed.Query?.method || parsed.Action?.method || "";
				console.log(`➡️  VR → Daemon: ${requestType} ${method}`.trim());
			} catch {
				const preview =
					messageStr.length > 150
						? messageStr.substring(0, 150) + "..."
						: messageStr;
				console.log(`➡️  VR → Daemon: ${preview}`);
			}

			// Create a new TCP connection for each request
			// (Daemon closes connection after each response)
			const tcpSocket = new Socket();

			tcpSocket.connect(DAEMON_PORT, DAEMON_HOST, () => {
				// Send request with newline (daemon expects newline-delimited JSON)
				tcpSocket.write(messageStr + "\n");
			});

			// Forward response from daemon to WebSocket client
			tcpSocket.on("data", (data) => {
				const lines = data
					.toString()
					.split("\n")
					.filter((line) => line.trim());
				for (const line of lines) {
					const preview =
						line.length > 150
							? line.substring(0, 150) + "..."
							: line;
					console.log(`⬅️  Daemon → VR: ${preview}`);
					ws.send(line);
				}
			});

			tcpSocket.on("error", (err) => {
				console.error("❌ TCP socket error:", err.message);
				ws.send(
					JSON.stringify({
						Error: { message: `Daemon error: ${err.message}` },
					}),
				);
			});

			tcpSocket.on("close", () => {
				// Connection closed after response - this is expected behavior
				tcpSocket.destroy();
			});
		},

		close(ws) {
			console.log("🔌 WebSocket client disconnected");
			// No persistent TCP connection to clean up
		},
	},
});

console.log(`\n✨ Proxy server ready!\n`);
console.log(`📋 IMPORTANT - Follow these steps in your VR browser:\n`);
console.log(`   1. Visit: https://<your-laptop-ip>:${PROXY_PORT}/test`);
console.log(`   2. Accept the certificate warning`);
console.log(`   3. Click "Test WebSocket" to verify connection`);
console.log(`   4. If test succeeds, you can load the VR app\n`);
console.log(`🌐 WebSocket URL: wss://<your-laptop-ip>:${PROXY_PORT}/ws\n`);
