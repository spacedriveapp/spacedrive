import { useEffect } from 'react';
import { emit, listen, Event } from '@tauri-apps/api/event';
import { useExplorerStore } from '../store/explorer';

export interface RustEvent {
  kind: string;
  data: any;
}

export function useGlobalEvents() {
  useEffect(() => {
    listen('message', (e: Event<RustEvent>) => {
      console.log({ e });

      switch (e.payload?.kind) {
        case 'FileTypeThumb':
          if (e.payload?.data.icon_created)
            useExplorerStore.getState().nativeIconUpdated(e.payload.data.file_id);
          break;

        default:
          break;
      }
    });
  }, []);
}
