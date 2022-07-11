import { BaseTransport } from '@sd/client';
import { ClientCommand, ClientQuery } from '@sd/core';
import SpacedriveInterface from '@sd/interface';
import React, { useEffect } from 'react';

const timeouts = [1000, 2000, 5000, 10000]; // In milliseconds

const randomId = () => Math.random().toString(36).slice(2);

// bind state to core via Tauri
class Transport extends BaseTransport {
	websocket: WebSocket;
	requestMap = new Map<string, (data: any) => void>();

	constructor() {
		super();
		this.websocket = new WebSocket(
			import.meta.env.VITE_SDSERVER_BASE_URL || 'ws://localhost:8080/ws'
		);
		this.attachEventListeners();
	}

	async reconnect(timeoutIndex = 0) {
		let timeout =
			(timeouts[timeoutIndex] ?? timeouts[timeouts.length - 1]) +
			(Math.floor(Math.random() * 5000 /* 5 Seconds */) + 1);

		setTimeout(() => {
			let ws = new WebSocket(import.meta.env.VITE_SDSERVER_BASE_URL || 'ws://localhost:8080/ws');
			new Promise(function (resolve, reject) {
				ws.addEventListener('open', () => resolve(null));
				ws.addEventListener('close', reject);
			})
				.then(() => {
					this.websocket = ws;
					this.attachEventListeners();
					console.log('Reconnected!');
				})
				.catch((err) => this.reconnect(timeoutIndex++));
		}, timeout);
	}

	attachEventListeners() {
		this.websocket.addEventListener('message', (event) => {
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

		this.websocket.addEventListener('close', () => {
			console.log('GONE');
			this.reconnect();
		});
	}

	async query(query: ClientQuery) {
		if (websocket.readyState == 0) {
			let resolve: () => void;
			const promise = new Promise((res) => {
				resolve = () => res(undefined);
			});
			// @ts-ignore
			websocket.addEventListener('open', resolve);
			await promise;
		}

		const id = randomId();
		let resolve: (data: any) => void;

		const promise = new Promise((res) => {
			resolve = res;
		});

		// @ts-ignore
		this.requestMap.set(id, resolve);

		this.websocket.send(JSON.stringify({ id, payload: { type: 'query', data: query } }));

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

		this.websocket.send(JSON.stringify({ id, payload: { type: 'command', data: command } }));

		return await promise;
	}
}

const transport = new Transport();

function App() {
	useEffect(() => {
		window.parent.postMessage('spacedrive-hello', '*');
	}, []);

	return (
		<div className="App">
			{/* <header className="App-header"></header> */}
			<SpacedriveInterface
				demoMode
				transport={transport}
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
