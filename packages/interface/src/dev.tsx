import React from 'react';
import ReactDOM from 'react-dom/client';
import './style.scss';
import App from './App';
import { ClientCommand, ClientQuery, CoreEvent } from '@sd/core';
import { BaseTransport } from '@sd/client';

// bind state to core via Tauri
class Transport extends BaseTransport {
  async query(query: ClientQuery) {
    // return await invoke('client_query_transport', { data: query });
  }
  async command(query: ClientCommand) {
    // return await invoke('client_command_transport', { data: query });
  }
}

const root = ReactDOM.createRoot(document.getElementById('root') as HTMLElement);
root.render(
  <React.StrictMode>
    <App
      transport={new Transport()}
      onCoreEvent={function (event: CoreEvent): void {}}
      platform={'browser'}
      convertFileSrc={function (url: string): string {
        return url;
      }}
      openDialog={function (options: {
        directory?: boolean | undefined;
      }): Promise<string | string[]> {
        return Promise.resolve('');
      }}
    />
  </React.StrictMode>
);
