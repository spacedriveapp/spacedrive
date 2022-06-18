import { BaseTransport } from '@sd/client';
import { ClientCommand, ClientQuery, CoreEvent } from '@sd/core';
import SpacedriveInterface from '@sd/interface';
import React, { useEffect } from 'react';

const websocket = new WebSocket(import.meta.env.VITE_SDSERVER_BASE_URL || 'ws://localhost:8080/ws');

const randomId = () => Math.random().toString(36).slice(2);

// bind state to core via Tauri
class Transport extends BaseTransport {
	requestMap = new Map<string, (data: any) => void>();

	constructor() {
		super();

		websocket.addEventListener('message', (event) => {
			if (!event.data) return;

			const { id, payload } = JSON.parse(event.data);

			const { type, data } = payload;
			if (type === 'event') {
				this.emit('core_event', data);
			} else if (type === 'query' || type === 'command') {
				if (this.requestMap.has(id)) {
					this.requestMap.get(id)?.(data);
					this.requestMap.delete(id);
				}
			}
		});
	}
	async query(query: ClientQuery) {
		const id = randomId();
		let resolve: (data: any) => void;

		const promise = new Promise((res) => {
			resolve = res;
		});

		// @ts-ignore
		this.requestMap.set(id, resolve);

		websocket.send(JSON.stringify({ id, payload: { type: 'query', data: query } }));

		return await promise;
	}
	async command(command: ClientCommand) {
		const id = randomId();
		let resolve: (data: any) => void;

		const promise = new Promise((res) => {
			resolve = res;
		});

		// @ts-ignore
		this.requestMap.set(id, resolve);

		websocket.send(JSON.stringify({ id, payload: { type: 'command', data: command } }));

		return await promise;
	}
}

function App() {
	useEffect(() => {
		window.parent.postMessage('spacedrive-hello', '*');
	}, []);

	return (
		<div className="App">
			{/* <header className="App-header"></header> */}
			<SpacedriveInterface
				demoMode
				transport={new Transport()}
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
		</div>
	);
}

export default App;
