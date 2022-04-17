import { useContext, useEffect } from 'react';
import { CoreEvent } from '@sd/core';
import { useQueryClient } from 'react-query';
import { useExplorerState } from '../components/file/FileList';
import { AppPropsContext } from '../App';

export function useCoreEvents() {
  const client = useQueryClient();
  const appPropsContext = useContext(AppPropsContext);

  const { addNewThumbnail } = useExplorerState();
  useEffect(() => {
    // check Tauri Event type
    // @ts-expect-error
    appPropsContext?.onCoreEvent((e: CoreEvent) => {
      switch (e?.key) {
        case 'NewThumbnail':
          addNewThumbnail(e.data.cas_id);
          break;
        case 'InvalidateQuery':
        case 'InvalidateQueryDebounced':
          let query = [e.data.key];
          // TODO: find a way to make params accessible in TS
          // also this method will only work for queries that use the whole params obj as the second key
          // @ts-expect-error
          if (e.data.params) {
            // @ts-expect-error
            query.push(e.data.params);
          }
          client.invalidateQueries(e.data.key);
          break;

        default:
          break;
      }
    });

    // listen('core_event', (e: { payload: CoreEvent }) => {
    // });
  }, []);
}
