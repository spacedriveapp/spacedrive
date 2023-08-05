import cryptoRandomString from 'crypto-random-string';
import { useCallback, useRef, useState } from 'react';

// NOTE: `crypto` module is not available in RN so this can't be in client
export const generatePassword = (length: number) =>
	cryptoRandomString({ length, type: 'ascii-printable' });

export function useForceUpdate() {
	const [, setTick] = useState(0);
	return useCallback(() => setTick((tick) => tick + 1), []);
}

export function usePreviousEqualCheck<T>(value: T) {
	const prev = useRef<T | undefined>();

	const equal = prev !== undefined && value === prev.current;

	prev.current = value;

	return equal;
}
