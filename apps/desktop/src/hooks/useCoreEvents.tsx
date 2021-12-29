import { useEffect } from 'react';
import { emit, listen, Event } from '@tauri-apps/api/event';
import { useExplorerStore } from '../store/explorer';
import {ClientEvent} from 'core/bindings/clientEvent';

export function useCoreEvents() {
  useEffect(() => {
    listen('core_event', (e: Event<ClientEvent>) => {
      console.log({ e });

      switch (e.payload?.type) {
        case 'NewFileTypeThumb':
          console.log(e.payload?.data.file_id);
          
          if (e.payload?.data.icon_created)
            useExplorerStore.getState().nativeIconUpdated(e.payload.data.file_id);
          break;

        default:
          break;
      }
    });
  }, []);
}
