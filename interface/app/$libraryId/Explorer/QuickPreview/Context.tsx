import { PropsWithChildren, RefObject, createContext, useContext, useRef } from 'react';

interface QuickPreviewContext {
    ref: RefObject<HTMLDivElement>;
}

const QuickPreviewContext = createContext<QuickPreviewContext | null>(null);

export const QuickPreviewContextProvider = ({ children }: PropsWithChildren) => {
    const ref = useRef<HTMLDivElement>(null);

    return (
        <QuickPreviewContext.Provider value={{ ref }}>
            {children}
            <div ref={ref} />
        </QuickPreviewContext.Provider>
    );
};

export const useQuickPreviewContext = () => {
    const context = useContext(QuickPreviewContext);

    if (!context) throw new Error('QuickPreviewContext.Provider not found!');

    return context;
};
