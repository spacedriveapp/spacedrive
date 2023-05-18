import { useSnapshot } from 'valtio';
import { valtioPersist } from '@sd/client';

export const explorerConfigStore = valtioPersist('explorer-config', {
	openOnDoubleClick: true
});

export function useExplorerConfigStore() {
	return useSnapshot(explorerConfigStore);
}

export function getExplorerConfigStore() {
	return explorerConfigStore;
}
