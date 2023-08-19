import { useSnapshot } from 'valtio';
import { DoubleClickAction, valtioPersist } from '@sd/client';

export const explorerConfigStore = valtioPersist('explorer-config', {
	openOnDoubleClick: 'openFile' as DoubleClickAction
});

export function useExplorerConfigStore() {
	return useSnapshot(explorerConfigStore);
}

export function getExplorerConfigStore() {
	return explorerConfigStore;
}
