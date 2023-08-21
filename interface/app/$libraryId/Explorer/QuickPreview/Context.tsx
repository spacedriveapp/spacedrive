import { PropsWithChildren, createContext, useContext, useState } from 'react';

interface QuickPreviewContext {
	ref: HTMLDivElement | null;
}

const QuickPreviewContext = createContext<QuickPreviewContext | null>(null);

export const QuickPreviewContextProvider = ({ children }: PropsWithChildren) => {
	const [ref, setRef] = useState<HTMLDivElement | null>(null);

	return (
		<QuickPreviewContext.Provider value={{ ref }}>
			{children}
			<div ref={setRef} />
		</QuickPreviewContext.Provider>
	);
};

export const useQuickPreviewContext = () => {
	const context = useContext(QuickPreviewContext);

	if (!context) throw new Error('QuickPreviewContext.Provider not found!');

	return context;
};
