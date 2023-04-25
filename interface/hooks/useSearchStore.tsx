import { proxy, useSnapshot } from 'valtio';

const searchStore = proxy({ isFocused: false });

export const useSearchStore = () => useSnapshot(searchStore);

export const getSearchStore = () => searchStore;
