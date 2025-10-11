import { createConnection, Socket } from 'net';
import { EventEmitter } from 'events';
import {
  Event,
  JobOutput,
  JobStatus,
  LibraryCreateInput,
  LibraryCreateOutput,
  LibraryInfo,
  JobListOutput
} from './types';

/**
 * Type-safe TypeScript client for Spacedrive daemon
 *
 * Provides a clean interface for executing queries, actions, and subscribing to events
 * from the Spacedrive core using Unix domain sockets.
 */
export class SpacedriveClient extends EventEmitter {
  private socketPath: string;

  constructor(socketPath: string = process.env.HOME + '/Library/Application Support/spacedrive/daemon/daemon.sock') {
    super();
    this.socketPath = socketPath;
  }

  // MARK: - Core API Methods

  /**
   * Execute a query operation
   */
  async executeQuery<Q, R>(query: Q, method: string): Promise<R> {
    console.log(`Executing query: ${method}`);

    const request = {
      JsonQuery: {
        method,
        payload: query
      }
    };

    const response = await this.sendRequest(request);

    if ('JsonOk' in response) {
      console.log('Query successful');
      return response.JsonOk;
    } else if ('Error' in response) {
      throw new Error(`Query failed: ${response.Error}`);
    } else {
      throw new Error(`Unexpected response to query: ${JSON.stringify(response)}`);
    }
  }

  /**
   * Execute an action operation
   */
  async executeAction<A, R>(action: A, method: string): Promise<R> {
    const request = {
      JsonAction: {
        method,
        payload: action
      }
    };

    const response = await this.sendRequest(request);

    if ('JsonOk' in response) {
      return response.JsonOk;
    } else if ('Error' in response) {
      throw new Error(`Action failed: ${response.Error}`);
    } else {
      throw new Error(`Unexpected response to action: ${JSON.stringify(response)}`);
    }
  }

  /**
   * Subscribe to events from the daemon
   */
  async subscribe(eventTypes: string[] = []): Promise<void> {
    console.log('Starting event subscription...');

    const socket = await this.createConnection();

    // Send subscription request
    const subscribeRequest = {
      Subscribe: {
        event_types: eventTypes,
        filter: null
      }
    };

    await this.sendRequestOverSocket(subscribeRequest, socket);

    // Listen for events
    socket.on('data', (data: Buffer) => {
      const lines = data.toString().split('\n').filter(line => line.trim());

      for (const line of lines) {
        try {
          const response = JSON.parse(line);

          if ('Event' in response) {
            const event: Event = response.Event;
            console.log('Received event:', event);
            this.emit('spacedrive-event', event);
          } else if (line.includes('Subscribed')) {
            console.log('Event subscription active');
            this.emit('subscribed');
          }
        } catch (error) {
          console.error('Failed to parse event:', error);
          console.error('Raw line:', line);
        }
      }
    });

    socket.on('error', (error) => {
      console.error('Socket error:', error);
      this.emit('error', error);
    });

    socket.on('close', () => {
      console.log('Socket closed');
      this.emit('disconnected');
    });
  }

  /**
   * Ping the daemon to test connectivity
   */
  async ping(): Promise<void> {
    console.log('Sending ping...');
    const response = await this.sendRequest('Ping');

    if (response === 'Pong') {
      console.log('Ping successful!');
    } else {
      throw new Error(`Unexpected ping response: ${JSON.stringify(response)}`);
    }
  }

  // MARK: - Convenience Methods

  /**
   * Create a library using generated types
   */
  async createLibrary(name: string, path?: string): Promise<LibraryCreateOutput> {
    const input: LibraryCreateInput = { name, path: path || null };
    return this.executeAction(input, 'action:libraries.create.input.v1');
  }

  /**
   * Get list of libraries
   */
  async getLibraries(includeStats: boolean = false): Promise<LibraryInfo[]> {
    const query = { include_stats: includeStats };
    return this.executeQuery(query, 'query:libraries.list.v1');
  }

  /**
   * Get list of jobs
   */
  async getJobs(status?: JobStatus): Promise<JobListOutput> {
    const query = { status: status || null };
    return this.executeQuery(query, 'query:jobs.list.v1');
  }

  // MARK: - Private Implementation

  private async sendRequest(request: any): Promise<any> {
    const socket = await this.createConnection();

    try {
      await this.sendRequestOverSocket(request, socket);
      return await this.readResponseFromSocket(socket);
    } finally {
      socket.destroy();
    }
  }

  private createConnection(): Promise<Socket> {
    return new Promise((resolve, reject) => {
      console.log(`Connecting to daemon at: ${this.socketPath}`);

      const socket = createConnection(this.socketPath);

      socket.on('connect', () => {
        console.log('Connected to daemon');
        resolve(socket);
      });

      socket.on('error', (error) => {
        console.error('Connection failed:', error);
        reject(error);
      });
    });
  }

  private sendRequestOverSocket(request: any, socket: Socket): Promise<void> {
    return new Promise((resolve, reject) => {
      const requestLine = JSON.stringify(request) + '\n';
      console.log(`Sending: ${requestLine.trim()}`);

      socket.write(requestLine, (error) => {
        if (error) {
          reject(error);
        } else {
          resolve();
        }
      });
    });
  }

  private readResponseFromSocket(socket: Socket): Promise<any> {
    return new Promise((resolve, reject) => {
      let buffer = '';

      const onData = (data: Buffer) => {
        buffer += data.toString();

        // Check for complete line
        const newlineIndex = buffer.indexOf('\n');
        if (newlineIndex !== -1) {
          const line = buffer.slice(0, newlineIndex).trim();
          console.log(`Received: ${line}`);

          try {
            const response = JSON.parse(line);
            socket.off('data', onData);
            resolve(response);
          } catch (error) {
            socket.off('data', onData);
            reject(new Error(`Failed to parse response: ${error}`));
          }
        }
      };

      socket.on('data', onData);

      socket.on('error', (error) => {
        socket.off('data', onData);
        reject(error);
      });
    });
  }
}

// Export all types for convenience
export * from './types';
