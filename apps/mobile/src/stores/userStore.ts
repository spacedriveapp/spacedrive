import { proxy, useSnapshot } from 'valtio';

export type User = {
	id: string;
	email: string;
	timeJoined: number;
	tenantIds: string[];
};

const state = {
	userInfo: undefined as User | undefined
};

const store = proxy({
	...state
});

// for reading
export const useUserStore = () => useSnapshot(store);

// for writing
export const getUserStore = () => store;
