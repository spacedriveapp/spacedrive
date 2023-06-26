import { RefObject, createContext, useContext } from "react";

interface QuickPreviewContext {
    ref: RefObject<HTMLDivElement>;
}

export const QuickPreviewContext = createContext<QuickPreviewContext | null>(null);

export const useQuickPreviewContext = () => {
    const context = useContext(QuickPreviewContext);

    if (!context)
        throw new Error("QuickPreviewContext.Provider not found!");

    return context;
};
