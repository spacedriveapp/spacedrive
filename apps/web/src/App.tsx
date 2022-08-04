import { WebsocketTransport, createClient } from '@rspc/client';
import { Operations, queryClient, rspc } from '@sd/client';
import SpacedriveInterface from '@sd/interface';
import React, { useEffect } from 'react';

const client = createClient<Operations>({
	transport: new WebsocketTransport(
		import.meta.env.VITE_SDSERVER_BASE_URL || 'ws://localhost:8080/rspcws'
	)
});

function App() {
	useEffect(() => {
		window.parent.postMessage('spacedrive-hello', '*');
	}, []);

	return (
		<div className="App">
			<rspc.Provider client={client} queryClient={queryClient}>
				<SpacedriveInterface
					demoMode
					platform={'browser'}
					convertFileSrc={function (url: string): string {
						return url;
					}}
					openDialog={function (options: {
						directory?: boolean | undefined;
					}): Promise<string | string[]> {
						return Promise.resolve([]);
					}}
				/>
			</rspc.Provider>
		</div>
	);
}

export default App;
