import React from 'react';
import ReactDOM from 'react-dom';
import App from './App';
import './style.css';

import { ClientCommand, ClientQuery } from '@sd/core';
import { BaseTransport, setTransport } from '@sd/state';
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

ReactDOM.render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
  document.getElementById('root')
);
