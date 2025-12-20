import type { Transport } from "@sd/ts-client";

export class HttpTransport implements Transport {
	constructor(private baseUrl: string) {}

	async sendRequest(request: any): Promise<any> {
		console.log("[HttpTransport] Sending request:", request);
		console.log("[HttpTransport] URL:", `${this.baseUrl}/rpc`);

		try {
			const response = await fetch(`${this.baseUrl}/rpc`, {
				method: "POST",
				headers: {
					"Content-Type": "application/json",
				},
				body: JSON.stringify(request),
			});

			console.log("[HttpTransport] Response status:", response.status);

			if (!response.ok) {
				const errorText = await response.text();
				console.error("[HttpTransport] Error response:", errorText);
				throw new Error(
					`HTTP error! status: ${response.status}, body: ${errorText}`,
				);
			}

			const data = await response.json();
			console.log("[HttpTransport] Response data:", data);
			return data;
		} catch (err) {
			console.error("[HttpTransport] Request failed:", err);
			throw err;
		}
	}

	async subscribe(callback: (event: any) => void): Promise<() => void> {
		// For now, no event subscription over HTTP
		// TODO: Implement WebSocket for events
		console.log("[HttpTransport] subscribe called (not implemented)");
		return () => {};
	}
}
