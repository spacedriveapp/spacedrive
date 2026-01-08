import { useEffect, useRef, createContext, useContext } from "react";
import { useTopBar, TopBarPriority } from "./Context";

const PositionContext = createContext<"left" | "center" | "right">("left");

export function useTopBarPosition() {
	return useContext(PositionContext);
}

export { PositionContext };

interface TopBarItemProps {
	id: string;
	label: string;
	priority?: TopBarPriority;
	onClick?: () => void;
	children: React.ReactNode;
}

export function TopBarItem({
	id,
	label,
	priority = "normal",
	onClick,
	children,
}: TopBarItemProps) {
	const { registerItem, unregisterItem } = useTopBar();
	const position = useTopBarPosition();

	// Register/unregister on mount/unmount
	useEffect(() => {
		registerItem({
			id,
			label,
			priority,
			position,
			onClick,
			element: children,
		});

		return () => unregisterItem(id);
	}, [id, registerItem, unregisterItem]);

	// Update element when children, label, priority, position, or onClick changes
	useEffect(() => {
		registerItem({
			id,
			label,
			priority,
			position,
			onClick,
			element: children,
		});
	}, [children, label, priority, position, onClick, id, registerItem]);

	// Don't render anything - items are rendered by TopBarSection
	return null;
}