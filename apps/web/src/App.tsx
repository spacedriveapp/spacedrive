import React from 'react';

import SpacedriveInterface from '@sd/interface';
import { ClientCommand, ClientQuery } from '@sd/core';
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

function App() {
  return (
    <div className="App">
      {/* <header className="App-header"></header> */}
      <SpacedriveInterface
        transport={new Transport()}
        onCoreEvent={function (event: any): void {
          return;
        }}
        platform={'browser'}
        convertFileSrc={function (url: string): string {
          return url;
        }}
        openDialog={function (options: { directory?: boolean }): Promise<string | string[]> {
          return Promise.resolve('');
        }}
      />
    </div>
  );
}

export default App;
