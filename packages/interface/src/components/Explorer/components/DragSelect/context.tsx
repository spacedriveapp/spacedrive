import { createContext, useContext, type ReactNode, type RefObject } from "react";
import type Selecto from "react-selecto";

interface DragSelectContextValue {
	selectoRef: RefObject<Selecto> | null;
	isWindows: boolean;
}

const DragSelectContext = createContext<DragSelectContextValue | null>(null);

export function useDragSelectContext() {
	const context = useContext(DragSelectContext);
	if (!context) {
		throw new Error("useDragSelectContext must be used within DragSelectProvider");
	}
	return context;
}

interface DragSelectProviderProps {
	children: ReactNode;
	selectoRef: RefObject<Selecto>;
}

export function DragSelectProvider({ children, selectoRef }: DragSelectProviderProps) {
	const isWindows = navigator.platform.toLowerCase().includes("win");

	return (
		<DragSelectContext.Provider value={{ selectoRef, isWindows }}>
			{children}
		</DragSelectContext.Provider>
	);
}
