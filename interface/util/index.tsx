import { lazy } from '@loadable/component';
import cryptoRandomString from 'crypto-random-string';
import { ReactNode } from 'react';

export function lazyEl(fn: Parameters<typeof lazy>[0]): ReactNode {
	const Element = lazy(fn);
	return <Element />;
}

// NOTE: `crypto` module is not available in RN so this can't be in client
export const generatePassword = (length: number) =>
	cryptoRandomString({ length, type: 'ascii-printable' });
