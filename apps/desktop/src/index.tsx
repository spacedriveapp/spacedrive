import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';

// import Spacedrive interface
import SpacedriveInterface, { Platform } from '@sd/interface';
import '@sd/interface/dist/style.css';

// import types from Spacedrive core (TODO: re-export from client would be cleaner)
import { ClientCommand, ClientQuery, CoreEvent } from '@sd/core';
// import Spacedrive JS client
import { BaseTransport } from '@sd/client';
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

function App() {
  function getPlatform(platform: string): Platform {
    switch (platform) {
      case 'darwin':
        return 'macOS';
      case 'win32':
        return 'windows';
      case 'linux':
        return 'linux';
      default:
        return 'browser';
    }
  }

  const [platform, setPlatform] = useState<Platform>('macOS');

  useEffect(() => {
    os.platform().then((platform) => setPlatform(getPlatform(platform)));
  }, []);

  return (
    <SpacedriveInterface
      transport={new Transport()}
      onCoreEvent={function (event: CoreEvent): void {
        return;
      }}
      platform={platform}
      convertFileSrc={function (url: string): string {
        return url;
      }}
      openDialog={function (options: { directory?: boolean | undefined }): Promise<void> {
        return Promise.resolve();
      }}
    />
  );
}

const root = createRoot(document.getElementById('root')!);

root.render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
