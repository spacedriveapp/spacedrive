#!/usr/bin/env bun

/**
 * Development script that:
 * 1. Builds the daemon (dev profile)
 * 2. Starts the daemon
 * 3. Waits for it to be ready
 * 4. Starts Vite dev server
 * 5. Cleans up daemon on exit
 */

import { spawn } from "child_process";
import { existsSync, unlinkSync } from "fs";
import { join, resolve, dirname } from "path";
import { homedir } from "os";
import { fileURLToPath } from "url";

// Get script directory
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Paths relative to this script (apps/tauri/scripts/)
// Script is at: PROJECT_ROOT/apps/tauri/scripts/
// So PROJECT_ROOT is: ../../../
const PROJECT_ROOT = resolve(__dirname, "../../../");
const DAEMON_BIN = join(PROJECT_ROOT, "target/debug/sd-daemon");
const DAEMON_PORT = 6969;
const DAEMON_ADDR = `127.0.0.1:${DAEMON_PORT}`;
const DATA_DIR = join(homedir(), "Library/Application Support/spacedrive");

let daemonProcess: any = null;
let viteProcess: any = null;
let startedDaemon = false;

// Cleanup function
function cleanup() {
	console.log("\nCleaning up...");

	if (viteProcess) {
		console.log("Stopping Vite...");
		viteProcess.kill();
	}

	if (daemonProcess && startedDaemon) {
		console.log("Stopping daemon (started by us)...");
		daemonProcess.kill();
	} else if (!startedDaemon) {
		console.log("Leaving existing daemon running...");
	}

	process.exit(0);
}

// Handle signals
process.on("SIGINT", cleanup);
process.on("SIGTERM", cleanup);

async function main() {
	console.log("Building daemon (dev profile)...");
	console.log("Project root:", PROJECT_ROOT);
	console.log("Daemon binary:", DAEMON_BIN);

	// Build daemon
	const build = spawn("cargo", ["build", "--bin", "sd-daemon"], {
		cwd: PROJECT_ROOT,
		stdio: "inherit",
	});

	await new Promise<void>((resolve, reject) => {
		build.on("exit", (code) => {
			if (code === 0) {
				resolve();
			} else {
				reject(new Error(`Daemon build failed with code ${code}`));
			}
		});
	});

	console.log("Daemon built successfully");

	// Check if daemon is already running by trying to connect to TCP port
	let daemonAlreadyRunning = false;
	console.log(`Checking if daemon is running on ${DAEMON_ADDR}...`);
	try {
		const { connect } = await import("net");
		await new Promise<void>((resolve, reject) => {
			const client = connect(DAEMON_PORT, "127.0.0.1");
			client.on("connect", () => {
				daemonAlreadyRunning = true;
				client.end();
				resolve();
			});
			client.on("error", () => {
				reject();
			});
			setTimeout(() => reject(), 1000);
		});
	} catch (e) {
		// Connection failed, daemon not running
		daemonAlreadyRunning = false;
	}

	if (daemonAlreadyRunning) {
		console.log("Daemon already running, will connect to existing instance");
		startedDaemon = false;
	} else {
		// Start daemon
		console.log("Starting daemon...");
		startedDaemon = true;

		// Verify binary exists
		if (!existsSync(DAEMON_BIN)) {
			throw new Error(`Daemon binary not found at: ${DAEMON_BIN}`);
		}

		daemonProcess = spawn(DAEMON_BIN, ["--data-dir", DATA_DIR], {
			cwd: PROJECT_ROOT,
			stdio: ["ignore", "pipe", "pipe"],
			env: {
				...process.env,
				DYLD_LIBRARY_PATH: join(PROJECT_ROOT, "apps/.deps/lib"),
			},
		});

		// Log daemon output
		daemonProcess.stdout.on("data", (data: Buffer) => {
			const lines = data.toString().trim().split("\n");
			for (const line of lines) {
				console.log(`[daemon] ${line}`);
			}
		});

		daemonProcess.stderr.on("data", (data: Buffer) => {
			const lines = data.toString().trim().split("\n");
			for (const line of lines) {
				console.log(`[daemon] ${line}`);
			}
		});

		// Wait for daemon to be ready
		console.log("Waiting for daemon to be ready...");
		for (let i = 0; i < 30; i++) {
			try {
				const { connect } = await import("net");
				await new Promise<void>((resolve, reject) => {
					const client = connect(DAEMON_PORT, "127.0.0.1");
					client.on("connect", () => {
						client.end();
						resolve();
					});
					client.on("error", reject);
					setTimeout(() => reject(), 500);
				});
				console.log(`Daemon ready at ${DAEMON_ADDR}`);
				break;
			} catch (e) {
				if (i === 29) {
					throw new Error("Daemon failed to start (connection not available)");
				}
				await new Promise((resolve) => setTimeout(resolve, 1000));
			}
		}
	}

	// Start Vite
	console.log("Starting Vite dev server...");
	viteProcess = spawn("bun", ["run", "dev"], {
		stdio: "inherit",
	});

	// Keep running
	await new Promise(() => {});
}

main().catch((error) => {
	console.error("Error:", error);
	cleanup();
	process.exit(1);
});
