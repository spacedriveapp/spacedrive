import {useCallback, useEffect, useRef, useState} from 'react';

type EventHandler = (data: unknown) => void;

type ConnectionState =
	| 'connecting'
	| 'connected'
	| 'reconnecting'
	| 'disconnected';

interface UseSpacebotEventSourceOptions {
	handlers: Record<string, EventHandler>;
	enabled?: boolean;
	onReconnect?: () => void;
}

const INITIAL_RETRY_MS = 1000;
const MAX_RETRY_MS = 30000;
const BACKOFF_MULTIPLIER = 2;

export function useSpacebotEventSource(
	url: string,
	{handlers, enabled = true, onReconnect}: UseSpacebotEventSourceOptions
) {
	const handlersRef = useRef(handlers);
	handlersRef.current = handlers;

	const onReconnectRef = useRef(onReconnect);
	onReconnectRef.current = onReconnect;

	const [connectionState, setConnectionState] =
		useState<ConnectionState>('connecting');
	const reconnectTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);
	const eventSourceRef = useRef<EventSource | null>(null);
	const retryDelayRef = useRef(INITIAL_RETRY_MS);
	const hadConnectionRef = useRef(false);

	const connect = useCallback(() => {
		if (eventSourceRef.current) {
			eventSourceRef.current.close();
		}

		setConnectionState(
			hadConnectionRef.current ? 'reconnecting' : 'connecting'
		);

		const source = new EventSource(url);
		eventSourceRef.current = source;

		source.onopen = () => {
			const wasReconnect = hadConnectionRef.current;
			hadConnectionRef.current = true;
			retryDelayRef.current = INITIAL_RETRY_MS;
			setConnectionState('connected');

			if (wasReconnect) {
				onReconnectRef.current?.();
			}
		};

		for (const eventType of Object.keys(handlersRef.current)) {
			source.addEventListener(eventType, (event: MessageEvent) => {
				try {
					const data = JSON.parse(event.data);
					handlersRef.current[eventType]?.(data);
				} catch {
					handlersRef.current[eventType]?.(event.data);
				}
			});
		}

		source.addEventListener('lagged', () => {
			onReconnectRef.current?.();
		});

		source.onerror = () => {
			source.close();
			setConnectionState('reconnecting');

			const delay = retryDelayRef.current;
			retryDelayRef.current = Math.min(
				delay * BACKOFF_MULTIPLIER,
				MAX_RETRY_MS
			);
			reconnectTimeout.current = setTimeout(connect, delay);
		};
	}, [url]);

	useEffect(() => {
		if (!enabled) {
			setConnectionState('disconnected');
			return;
		}

		connect();

		return () => {
			if (reconnectTimeout.current) {
				clearTimeout(reconnectTimeout.current);
			}
			eventSourceRef.current?.close();
		};
	}, [connect, enabled]);

	return {connectionState};
}
