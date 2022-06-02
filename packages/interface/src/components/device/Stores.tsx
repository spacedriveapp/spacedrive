import create from 'zustand';

const getLocalStorage = (key: string) => JSON.parse(window.localStorage.getItem(key) || '{}');
const setLocalStorage = (key: string, value: any) =>
	window.localStorage.setItem(key, JSON.stringify(value));

type NodeState = {
	isExperimental: boolean;
	setIsExperimental: (experimental: boolean) => void;
};

export const useNodeStore = create<NodeState>((set) => ({
	isExperimental: (getLocalStorage('isExperimental') as boolean) === true || false,
	setIsExperimental: (experimental: boolean) =>
		set((state) => {
			setLocalStorage('isExperimental', experimental);
			return { ...state, isExperimental: experimental };
		})
}));
