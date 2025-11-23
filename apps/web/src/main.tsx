import React from "react";
import ReactDOM from "react-dom/client";
import { PlatformProvider } from "@sd/interface/platform";
import { Explorer } from "@sd/interface";
import { platform } from "./platform";
import "@sd/interface/styles.css";

/**
 * Web entry point for Spacedrive server interface
 */
function App() {
	return (
		<PlatformProvider platform={platform}>
			<Explorer />
		</PlatformProvider>
	);
}

ReactDOM.createRoot(document.getElementById("root")!).render(
	<React.StrictMode>
		<App />
	</React.StrictMode>
);
