import { Transport, UnixSocketTransport } from './transport';

/**
 * Main client for interacting with the Spacedrive daemon
 *
 * This client provides a clean, type-safe interface for executing queries,
 * actions, and subscribing to events from the Spacedrive core.
 */
export class SpacedriveClient {
    private transport: Transport;

    /**
     * Initialize a new Spacedrive client
     * @param socketPath Path to the Unix domain socket for the daemon
     */
    constructor(socketPath: string) {
        this.transport = new UnixSocketTransport(socketPath);
    }

    /**
     * Execute a query operation
     * @param query The query input (can be empty object for parameterless queries)
     * @param method The method identifier (e.g., "query:core.status.v1")
     * @returns Promise resolving to the query result
     */
    async executeQuery<Q, R>(
        query: Q,
        method: string
    ): Promise<R> {
        // 1. Serialize query to JSON
        const queryData = JSON.stringify(query);

        // 2. Create daemon request
        const request: DaemonRequest = {
            Query: {
                method,
                payload: Buffer.from(queryData).toString('base64')
            }
        };

        // 3. Send to daemon and get response
        const response = await this.transport.sendRequest(request);

        // 4. Handle response
        if ('Success' in response) {
            return JSON.parse(response.Success) as R;
        } else if ('Error' in response) {
            throw SpacedriveError.daemonError(response.Error);
        } else {
            throw SpacedriveError.invalidResponse('Unexpected event response to query');
        }
    }

    /**
     * Execute an action operation
     * @param action The action input
     * @param method The method identifier (e.g., "action:libraries.create.input.v1")
     * @returns Promise resolving to the action result
     */
    async executeAction<A, R>(
        action: A,
        method: string
    ): Promise<R> {
        // 1. Serialize action to JSON
        const actionData = JSON.stringify(action);

        // 2. Create daemon request
        const request: DaemonRequest = {
            Action: {
                method,
                payload: Buffer.from(actionData).toString('base64')
            }
        };

        // 3. Send to daemon and get response
        const response = await this.transport.sendRequest(request);

        // 4. Handle response
        if ('Success' in response) {
            return JSON.parse(response.Success) as R;
        } else if ('Error' in response) {
            throw SpacedriveError.daemonError(response.Error);
        } else {
            throw SpacedriveError.invalidResponse('Unexpected event response to action');
        }
    }

    /**
     * Subscribe to events from the daemon
     * @param eventTypes Array of event type names to subscribe to
     * @returns AsyncGenerator yielding events as they arrive
     */
    async* subscribe(
        eventTypes: string[] = []
    ): AsyncGenerator<SpacedriveEvent, void, unknown> {
        // 1. Create subscription request
        const request: DaemonRequest = {
            Subscribe: {
                event_types: eventTypes,
                filter: null
            }
        };

        // 2. Start subscription and yield events
        const eventStream = this.transport.subscribe(request);

        for await (const response of eventStream) {
            if ('Event' in response) {
                yield JSON.parse(response.Event) as SpacedriveEvent;
            }
        }
    }

    /**
     * Ping the daemon to test connectivity
     */
    async ping(): Promise<void> {
        const response = await this.transport.sendRequest('Ping');

        if ('Success' in response) {
            return;
        } else if ('Error' in response) {
            throw SpacedriveError.daemonError(`Ping failed: ${response.Error}`);
        } else {
            throw SpacedriveError.invalidResponse('Unexpected event response to ping');
        }
    }
}

// Daemon Protocol Types

/**
 * Request types that match the Rust daemon protocol
 */
type DaemonRequest =
    | 'Ping'
    | { Action: { method: string; payload: string } }
    | { Query: { method: string; payload: string } }
    | { Subscribe: { event_types: string[]; filter: EventFilter | null } }
    | 'Unsubscribe'
    | 'Shutdown';

/**
 * Response types that match the Rust daemon protocol
 */
type DaemonResponse =
    | { Success: string }
    | { Error: string }
    | { Event: string };

/**
 * Event filter for subscriptions
 */
interface EventFilter {
    library_id?: string;
    job_id?: string;
    device_id?: string;
}

/**
 * Errors that can occur when using the Spacedrive client
 */
export class SpacedriveError extends Error {
    constructor(
        message: string,
        public readonly type: 'connection' | 'serialization' | 'daemon' | 'invalid_response' = 'daemon'
    ) {
        super(message);
        this.name = 'SpacedriveError';
    }

    static connectionFailed(message: string): SpacedriveError {
        return new SpacedriveError(message, 'connection');
    }

    static serializationError(message: string): SpacedriveError {
        return new SpacedriveError(message, 'serialization');
    }

    static daemonError(message: string): SpacedriveError {
        return new SpacedriveError(message, 'daemon');
    }

    static invalidResponse(message: string): SpacedriveError {
        return new SpacedriveError(message, 'invalid_response');
    }
}

// Placeholder event type until types.ts is generated
export interface SpacedriveEvent {
    // This will be replaced by the generated Event type
}

/**
 * Convenience methods for common operations
 */
export class SpacedriveClientExamples {
    constructor(private client: SpacedriveClient) {}

    /**
     * Get core status - demonstrates real type-safe API usage
     * Once types.ts is generated, this can use the actual OutputProperties type
     */
    async getCoreStatus(): Promise<any> {
        return await this.client.executeQuery({}, 'query:core.status.v1');
    }

    /**
     * Create a library - demonstrates action usage
     * Once types.ts is generated, this can use the actual LibraryCreateInput/Output types
     */
    async createLibrary(name: string, path?: string): Promise<any> {
        const input = { name, path };
        return await this.client.executeAction(input, 'action:libraries.create.input.v1');
    }

    /**
     * Ping the daemon to test connectivity
     */
    async ping(): Promise<void> {
        return await this.client.ping();
    }
}
