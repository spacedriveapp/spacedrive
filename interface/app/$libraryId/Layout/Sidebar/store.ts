import { proxy, useSnapshot } from 'valtio';

//This store is being used
//to record the state of the sidebar for specific behaviours i.e pinning

const store = proxy({
	pinJobManager: false as boolean
});

export const useSidebarStore = () => useSnapshot(store);
export const getSidebarStore = () => store;
