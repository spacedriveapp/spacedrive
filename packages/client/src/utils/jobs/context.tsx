import { createContext, PropsWithChildren, useContext, useRef } from 'react';

import { JobProgressEvent } from '../../core';

const JobManagerContext = createContext<ReturnType<typeof useValue> | null>(null);

const useValue = () => {
	const cachedJobProgress = useRef<Record<string, JobProgressEvent>>({});

	return {
		cachedJobProgress
	};
};

export const JobManagerContextProvider = (props: PropsWithChildren) => {
	const value = useValue();
	return <JobManagerContext.Provider value={value}>{props.children}</JobManagerContext.Provider>;
};

export const useJobManagerContext = () => {
	const ctx = useContext(JobManagerContext);

	if (ctx === null) throw new Error('JobManagerContext.Provider not found');

	return ctx;
};
