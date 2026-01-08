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
	submenuContent?: React.ReactNode;
}

export function TopBarItem({
	id,
	label,
	priority = "normal",
	onClick,
	children,
	submenuContent,
}: TopBarItemProps) {
	const { registerItem, unregisterItem } = useTopBar();
	const position = useTopBarPosition();

	// Register on mount, update when props change, unregister on unmount
	// Note: children and submenuContent should be memoized by parent to prevent infinite loops
	useEffect(() => {
		registerItem({
			id,
			label,
			priority,
			position,
			onClick,
			element: children,
			submenuContent,
		});
	}, [id, label, priority, position, onClick, registerItem, children, submenuContent]);

	// Unregister on unmount
	useEffect(() => {
		return () => unregisterItem(id);
	}, [id, unregisterItem]);

	// Don't render anything - items are rendered by TopBarSection
	return null;
}