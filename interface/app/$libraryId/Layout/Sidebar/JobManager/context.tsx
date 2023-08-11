import { PropsWithChildren, createContext, useContext, useRef } from 'react';
import { JobProgressEvent } from '@sd/client';

const JobManagerContext = createContext<ReturnType<typeof useValue> | null>(null);

// Custom hook allows createContext to infer type
const useValue = () => {
	const cachedJobProgress = useRef(new Map<string, JobProgressEvent>());

	// Would usually useMemo here but there's no functions so referential stability doesn't matter
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
