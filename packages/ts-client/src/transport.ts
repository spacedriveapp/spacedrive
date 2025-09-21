import * as net from 'net';

/**
 * Transport interface for communicating with the Spacedrive daemon
 */
export interface Transport {
    sendRequest(request: any): Promise<any>;
    subscribe(request: any): AsyncGenerator<any, void, unknown>;
}

/**
 * Unix domain socket transport for local daemon communication
 */
export class UnixSocketTransport implements Transport {
    constructor(private socketPath: string) {}

    /**
     * Send a single request to the daemon
     * @param request The request to send
     * @returns Promise resolving to the response
     */
    async sendRequest(request: any): Promise<any> {
        return new Promise((resolve, reject) => {
            const socket = net.createConnection(this.socketPath);

            socket.on('connect', () => {
                // Send JSON request with newline delimiter
                const requestData = JSON.stringify(request) + '\n';
                socket.write(requestData);
                socket.end();
            });

            let responseData = '';
            socket.on('data', (chunk) => {
                responseData += chunk.toString();
            });

            socket.on('end', () => {
                try {
                    // Remove trailing newline and parse
                    const cleanData = responseData.trim();
                    const response = JSON.parse(cleanData);
                    resolve(response);
                } catch (error) {
                    reject(new Error(`Failed to parse response: ${error}`));
                }
            });

            socket.on('error', (error) => {
                reject(new Error(`Socket error: ${error.message}`));
            });
        });
    }

    /**
     * Subscribe to events from the daemon
     * @param request The subscription request
     * @returns AsyncGenerator yielding events as they arrive
     */
    async* subscribe(request: any): AsyncGenerator<any, void, unknown> {
        const socket = net.createConnection(this.socketPath);

        await new Promise<void>((resolve, reject) => {
            socket.on('connect', () => {
                // Send subscription request with newline delimiter
                const requestData = JSON.stringify(request) + '\n';
                socket.write(requestData);
                resolve();
            });
            socket.on('error', reject);
        });

        let buffer = '';

        for await (const chunk of socket) {
            buffer += chunk.toString();

            // Process complete JSON messages (line-delimited)
            const lines = buffer.split('\n');
            buffer = lines.pop() || ''; // Keep incomplete line in buffer

            for (const line of lines) {
                if (line.trim()) {
                    try {
                        const response = JSON.parse(line);
                        yield response;
                    } catch (error) {
                        console.error('Failed to parse event:', error);
                    }
                }
            }
        }
    }
}
