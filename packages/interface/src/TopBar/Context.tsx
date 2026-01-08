import { createContext, useContext, useState, useCallback, useRef } from "react";

export type TopBarPriority = "high" | "normal" | "low";
export type TopBarPosition = "left" | "center" | "right";

export interface TopBarItem {
	id: string;
	label: string;
	priority: TopBarPriority;
	position: TopBarPosition;
	width: number;
	onClick?: () => void;
	element: React.ReactNode;
	elementVersion: number;
}

interface TopBarContextValue {
	items: Map<string, TopBarItem>;
	visibleItems: Set<string>;
	overflowItems: Map<TopBarPosition, TopBarItem[]>;

	registerItem: (item: Omit<TopBarItem, "width">) => void;
	unregisterItem: (id: string) => void;
	updateItemWidth: (id: string, width: number) => void;

	leftContainerRef: React.RefObject<HTMLDivElement> | null;
	rightContainerRef: React.RefObject<HTMLDivElement> | null;
	setLeftContainerRef: (ref: React.RefObject<HTMLDivElement>) => void;
	setRightContainerRef: (ref: React.RefObject<HTMLDivElement>) => void;

	recalculate: () => void;
}

const TopBarContext = createContext<TopBarContextValue | null>(null);

export function TopBarProvider({ children }: { children: React.ReactNode }) {
	const [items, setItems] = useState<Map<string, TopBarItem>>(new Map());
	const [visibleItems, setVisibleItemsState] = useState<Set<string>>(new Set());
	const [overflowItems, setOverflowItemsState] = useState<Map<TopBarPosition, TopBarItem[]>>(new Map());
	const [leftContainerRef, setLeftContainerRef] = useState<React.RefObject<HTMLDivElement> | null>(null);
	const [rightContainerRef, setRightContainerRef] = useState<React.RefObject<HTMLDivElement> | null>(null);
	const [recalculationTrigger, setRecalculationTrigger] = useState(0);
	const elementsRef = useRef<Map<string, React.ReactNode>>(new Map());

	const registerItem = useCallback((item: Omit<TopBarItem, "width" | "elementVersion">) => {
		// Store element in ref (doesn't trigger re-render)
		elementsRef.current.set(item.id, item.element);

		setItems(prev => {
			const existingItem = prev.get(item.id);

			// Check if structural properties changed
			const structureChanged = !existingItem ||
				existingItem.label !== item.label ||
				existingItem.priority !== item.priority ||
				existingItem.position !== item.position;

			// Only update state if structure changed
			if (!structureChanged) {
				// Just update the element in the existing item without triggering recalc
				const newItems = new Map(prev);
				newItems.set(item.id, {
					...existingItem,
					element: item.element,
					onClick: item.onClick,
				});
				return newItems;
			}

			// Structure changed - full update
			const newItems = new Map(prev);
			newItems.set(item.id, {
				...item,
				width: existingItem?.width || 0,
				elementVersion: 0
			});

			// Initially add to visible so it can be measured
			if (!existingItem) {
				setVisibleItemsState(prev2 => {
					const newVisible = new Set(prev2);
					newVisible.add(item.id);
					return newVisible;
				});
			}

			return newItems;
		});
	}, []);

	const unregisterItem = useCallback((id: string) => {
		setItems(prev => {
			const newItems = new Map(prev);
			newItems.delete(id);
			return newItems;
		});
		setVisibleItemsState(prev => {
			const newVisible = new Set(prev);
			newVisible.delete(id);
			return newVisible;
		});
	}, []);

	const updateItemWidth = useCallback((id: string, width: number) => {
		setItems(prev => {
			const item = prev.get(id);
			if (!item || item.width === width) return prev;

			const newItems = new Map(prev);
			newItems.set(id, { ...item, width });

			// Trigger recalculation only when we actually update
			setRecalculationTrigger(t => t + 1);

			return newItems;
		});
	}, []);

	const recalculate = useCallback(() => {
		setRecalculationTrigger(prev => prev + 1);
	}, []);

	return (
		<TopBarContext.Provider
			value={{
				items,
				visibleItems,
				overflowItems,
				registerItem,
				unregisterItem,
				updateItemWidth,
				leftContainerRef,
				rightContainerRef,
				setLeftContainerRef,
				setRightContainerRef,
				recalculate,
			}}
		>
			<TopBarInternalProvider
				setVisibleItems={setVisibleItemsState}
				setOverflowItems={setOverflowItemsState}
				recalculationTrigger={recalculationTrigger}
			>
				{children}
			</TopBarInternalProvider>
		</TopBarContext.Provider>
	);
}

// Internal context for state setters
interface TopBarInternalContextValue {
	setVisibleItems: React.Dispatch<React.SetStateAction<Set<string>>>;
	setOverflowItems: React.Dispatch<React.SetStateAction<Map<TopBarPosition, TopBarItem[]>>>;
	recalculationTrigger: number;
}

const TopBarInternalContext = createContext<TopBarInternalContextValue | null>(null);

function TopBarInternalProvider({
	children,
	setVisibleItems,
	setOverflowItems,
	recalculationTrigger,
}: {
	children: React.ReactNode;
	setVisibleItems: React.Dispatch<React.SetStateAction<Set<string>>>;
	setOverflowItems: React.Dispatch<React.SetStateAction<Map<TopBarPosition, TopBarItem[]>>>;
	recalculationTrigger: number;
}) {
	return (
		<TopBarInternalContext.Provider value={{ setVisibleItems, setOverflowItems, recalculationTrigger }}>
			{children}
		</TopBarInternalContext.Provider>
	);
}

export function useTopBar() {
	const context = useContext(TopBarContext);
	if (!context) {
		throw new Error("useTopBar must be used within TopBarProvider");
	}
	return context;
}

export function useTopBarInternal() {
	const context = useContext(TopBarInternalContext);
	if (!context) {
		throw new Error("useTopBarInternal must be used within TopBarProvider");
	}
	return context;
}