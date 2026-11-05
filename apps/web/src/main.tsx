import React from "react";
import ReactDOM from "react-dom/client";
import {
	PlatformProvider,
	ServerProvider,
	Settings,
	Shell,
	SpacedriveProvider,
} from "@sd/interface";
import type { Platform } from "@sd/interface/platform";
import { SpacedriveClient, HttpTransport } from "@sd/ts-client";
import { platform } from "./platform";
import "./index.css";
import "@sd/interface/styles.css";

// Talk to sd-server's /rpc endpoint on the same origin the page was loaded from.
// This works both standalone (browser → sd-server) and embedded inside an iframe.
const client = new SpacedriveClient(new HttpTransport());

// The desktop app opens Settings in its own window; a browser gets a tab on
// the same server, told which library this one is showing.
const webPlatform: Platform = {
	...platform,
	async showWindow(window: { type: string; page?: string }) {
		if (window.type !== "Settings") {
			throw new Error(`The web client has no ${window.type} window`);
		}
		const library = client.getCurrentLibraryId();
		const query = library ? `?library=${encodeURIComponent(library)}` : "";
		globalThis.open(`/settings/${window.page ?? "general"}${query}`, "_blank");
	},
};

function App() {
	if (window.location.pathname.startsWith("/settings")) {
		const library = new URLSearchParams(window.location.search).get("library");
		if (library) client.setCurrentLibrary(library, false);
		return (
			<PlatformProvider platform={webPlatform}>
				<SpacedriveProvider client={client}>
					<ServerProvider>
						<Settings />
					</ServerProvider>
				</SpacedriveProvider>
			</PlatformProvider>
		);
	}

	return (
		<PlatformProvider platform={webPlatform}>
			<Shell client={client} />
		</PlatformProvider>
	);
}

ReactDOM.createRoot(document.getElementById("root")!).render(
	<React.StrictMode>
		<App />
	</React.StrictMode>
);
