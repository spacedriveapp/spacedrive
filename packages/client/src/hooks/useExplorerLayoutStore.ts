import { valtioPersist } from "../lib";
import { useSnapshot } from 'valtio';

const explorerLayoutStore = valtioPersist('sd-explorer-layout', {
	showPathBar: true,
})

export function useExplorerLayoutStore() {
	return useSnapshot(explorerLayoutStore);
}

export function getExplorerLayoutStore() {
	return explorerLayoutStore;
}
