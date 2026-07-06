import React from "react";
import ReactDOM from "react-dom/client";
import { ErrorBoundary, PlatformProvider, Shell } from "@sd/interface";
import { SpacedriveClient, HttpTransport } from "@sd/ts-client";
import { platform } from "./platform";
import "./index.css";
import "@sd/interface/styles.css";

// Scope web-only CSS overrides (dialog visibility, etc.)
document.documentElement.dataset.sdPlatform = "web";

// Talk to sd-server's /rpc endpoint on the same origin the page was loaded from.
const client = new SpacedriveClient(new HttpTransport());

async function bootstrapLibrary(activeClient: SpacedriveClient) {
	if (activeClient.getCurrentLibraryId()) return;
	try {
		const libraries = await activeClient.execute<
			{ include_stats: boolean },
			Array<{ id: string }>
		>("query:libraries.list", { include_stats: false });
		if (libraries[0]?.id) {
			activeClient.setCurrentLibrary(libraries[0].id);
		}
	} catch (err) {
		console.error("[web] Failed to bootstrap library:", err);
	}
}

void bootstrapLibrary(client);

function App() {
	return (
		<PlatformProvider platform={platform}>
			<Shell client={client} />
		</PlatformProvider>
	);
}

ReactDOM.createRoot(document.getElementById("root")!).render(
	<React.StrictMode>
		<ErrorBoundary>
			<App />
		</ErrorBoundary>
	</React.StrictMode>
);