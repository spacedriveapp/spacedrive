import { lazy } from '@loadable/component';
import { ReactNode } from 'react';

export function lazyEl(fn: Parameters<typeof lazy>[0]): ReactNode {
	const Element = lazy(fn);
	return <Element />;
}
