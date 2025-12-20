/**
 * Configuration for VR app connection to Spacedrive daemon proxy
 *
 * Replace LAPTOP_IP with your laptop's local IP address (e.g., "192.168.0.91")
 * Run `ifconfig` (macOS/Linux) or `ipconfig` (Windows) to find your IP
 */

// Replace this with your laptop's IP address on the local network
const LAPTOP_IP = "192.168.0.91";

/**
 * WebSocket URL for daemon RPC communication
 * Uses secure WebSocket (wss://) because the page is loaded over HTTPS
 */
export const PROXY_WS_URL = `wss://${LAPTOP_IP}:8080/ws`;

/**
 * HTTP URL for sidecar files (thumbnails, thumbstrips, etc.)
 * Uses HTTPS because the page is loaded over HTTPS
 */
export const PROXY_HTTP_URL = `https://${LAPTOP_IP}:8080`;
