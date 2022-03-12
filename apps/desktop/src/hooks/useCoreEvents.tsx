import { useEffect } from 'react';
import { emit, listen, Event } from '@tauri-apps/api/event';
import { useExplorerStore } from '../store/explorer';
import { ClientEvent } from '@sd/core';

export function useCoreEvents() {
  useEffect(() => {
    listen('core_event', (e: Event<ClientEvent>) => {
      console.log({ e });

      switch (e.payload?.key) {
        case 'new_file_type_thumb':
          console.log(e.payload?.payload.file_id);

          if (e.payload?.payload.icon_created)
            useExplorerStore.getState().nativeIconUpdated(e.payload.payload.file_id);
          break;

        default:
          break;
      }
    });
  }, []);
}
