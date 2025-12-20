import { useMemo } from "react";
import { SpacedriveVR } from "./components/SpacedriveVR";
import { SpacedriveClient } from "@sd/ts-client";
import { SpacedriveProvider } from "@sd/interface";
import { WebSocketTransport } from "./websocketTransport";
import { PROXY_WS_URL } from "./config";

export function App() {
	const client = useMemo(() => {
		const transport = new WebSocketTransport(PROXY_WS_URL);
		return new SpacedriveClient(transport);
	}, []);

	return (
		<SpacedriveProvider client={client}>
			<SpacedriveVR />
		</SpacedriveProvider>
	);
}
