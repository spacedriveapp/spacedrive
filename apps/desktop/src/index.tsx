import React from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import './style.css';

import { ClientCommand, ClientQuery } from '../../../core';
import { BaseTransport, setTransport } from '@sd/client';
import { invoke } from '@tauri-apps/api';

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
    <App />
  </React.StrictMode>
);
