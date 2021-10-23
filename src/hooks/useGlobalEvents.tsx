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
          useExplorerStore
            .getState()
            .tempInjectThumb(e.payload.data.file_id, e.payload.data.thumbnail_b64);
          break;

        default:
          break;
      }
    });
  }, []);
}
