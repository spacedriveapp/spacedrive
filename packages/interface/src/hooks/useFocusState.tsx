import { useEffect, useState } from 'react';

export function useFocusState() {
	const [focused, setFocused] = useState(true);
	const focus = () => setFocused(true);
	const blur = () => setFocused(false);
	useEffect(() => {
		window.addEventListener('focus', focus);
		window.addEventListener('blur', blur);
		return () => {
			window.removeEventListener('focus', focus);
			window.removeEventListener('blur', blur);
		};
	}, []);

	return [focused];
}
