import { createContext, useContext, useState, useCallback } from "react";

interface TopBarContextValue {
	leftRef: React.RefObject<HTMLDivElement> | null;
	centerRef: React.RefObject<HTMLDivElement> | null;
	rightRef: React.RefObject<HTMLDivElement> | null;
	setLeftRef: (ref: React.RefObject<HTMLDivElement>) => void;
	setCenterRef: (ref: React.RefObject<HTMLDivElement>) => void;
	setRightRef: (ref: React.RefObject<HTMLDivElement>) => void;
}

const TopBarContext = createContext<TopBarContextValue | null>(null);

export function TopBarProvider({ children }: { children: React.ReactNode }) {
	const [leftRef, setLeftRef] = useState<React.RefObject<HTMLDivElement> | null>(null);
	const [centerRef, setCenterRef] = useState<React.RefObject<HTMLDivElement> | null>(null);
	const [rightRef, setRightRef] = useState<React.RefObject<HTMLDivElement> | null>(null);

	return (
		<TopBarContext.Provider
			value={{
				leftRef,
				centerRef,
				rightRef,
				setLeftRef,
				setCenterRef,
				setRightRef,
			}}
		>
			{children}
		</TopBarContext.Provider>
	);
}

export function useTopBar() {
	const context = useContext(TopBarContext);
	if (!context) {
		throw new Error("useTopBar must be used within TopBarProvider");
	}
	return context;
}
