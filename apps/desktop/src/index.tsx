import React from 'react';
import { createRoot } from 'react-dom/client';

// import Spacedrive interface
import SpacedriveInterface from '@sd/interface';
import '@sd/interface/dist/style.css';

// import types from Spacedrive core (TODO: re-export from client would be cleaner)
import { ClientCommand, ClientQuery, CoreEvent } from '@sd/core';
// import Spacedrive JS client
import { BaseTransport, setTransport } from '@sd/client';
// import tauri apis
import { invoke, os } from '@tauri-apps/api';

// bind state to core via Tauri
class Transport extends BaseTransport {
  async query(query: ClientQuery) {
    return await invoke('client_query_transport', { data: query });
  }
  async command(query: ClientCommand) {
    return await invoke('client_command_transport', { data: query });
  }
}
setTransport(new Transport());

const root = createRoot(document.getElementById('root')!);

root.render(
  <React.StrictMode>
    <SpacedriveInterface
      onCoreEvent={function (event: CoreEvent): void {
        return;
      }}
      //@ts-expect-error
      platform={os.platform()}
      convertFileSrc={function (url: string): string {
        return url;
      }}
      openDialog={function (options: { directory?: boolean | undefined }): Promise<void> {
        return Promise.resolve();
      }}
    />
  </React.StrictMode>
);
