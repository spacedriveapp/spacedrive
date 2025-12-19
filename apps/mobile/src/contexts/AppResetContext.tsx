import { createContext, useContext } from "react";

interface AppResetContextType {
	resetApp: () => void;
}

export const AppResetContext = createContext<AppResetContextType | null>(null);

export function useAppReset() {
	const context = useContext(AppResetContext);
	if (!context) {
		throw new Error("useAppReset must be used within AppResetContext.Provider");
	}
	return context;
}
