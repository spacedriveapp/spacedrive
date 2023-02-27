import { lazy } from '@loadable/component';
import { ReactNode } from 'react';
import { useParams } from 'react-router';

export function lazyEl(fn: Parameters<typeof lazy>[0]): ReactNode {
	const Element = lazy(fn);
	return <Element />;
}

export function useLibraryId() {
	return useParams<{ libraryId?: string }>().libraryId;
}
