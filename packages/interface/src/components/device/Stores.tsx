import { useState } from 'react';

const getLocalStorage = (key: string) => JSON.parse(window.localStorage.getItem(key) || '{}');
const setLocalStorage = (key: string, value: any) =>
	window.localStorage.setItem(key, JSON.stringify(value));

export function useNodeStore() {
	const [state, setState] = useState(
		(getLocalStorage('isExperimental') as boolean) === true || false
	);

	return {
		isExperimental: state,
		setIsExperimental: (experimental: boolean) => {
			setLocalStorage('isExperimental', experimental);
			setState(experimental);
		}
	};
}
