/**
 * TabView - Placeholder for future per-tab router isolation
 *
 * Currently unused. The MVP implementation uses a single shared router.
 * This component will be used in Phase 2 when each tab gets its own router instance.
 */

interface TabViewProps {
	isActive: boolean;
	children: React.ReactNode;
}

export function TabView({ isActive, children }: TabViewProps) {
	return (
		<div
			style={{ display: isActive ? "flex" : "none" }}
			className="flex-1 overflow-hidden"
		>
			{children}
		</div>
	);
}
