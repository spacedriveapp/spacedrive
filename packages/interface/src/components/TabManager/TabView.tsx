import { useMemo } from "react";
import { RouterProvider } from "react-router-dom";
import type { Tab } from "./TabManagerContext";

interface TabViewProps {
	tab: Tab;
	isActive: boolean;
	children: (isActive: boolean) => React.ReactNode;
}

export function TabView({ tab, isActive, children }: TabViewProps) {
	const content = useMemo(() => children(isActive), [children, isActive]);

	return (
		<div
			style={{ display: isActive ? "flex" : "none" }}
			className="flex-1 overflow-hidden"
		>
			<RouterProvider router={tab.router}>{content}</RouterProvider>
		</div>
	);
}
