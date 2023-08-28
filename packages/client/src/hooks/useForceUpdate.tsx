import { useCallback, useState } from 'react';

export function useForceUpdate() {
	const [, setTick] = useState(0);
	return useCallback(() => setTick((tick) => tick + 1), []);
}
