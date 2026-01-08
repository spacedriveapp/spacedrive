import { memo } from "react";
import { TopBarSection } from "./Section";
import { useOverflowCalculation } from "./useOverflowCalculation";

interface TopBarProps {
	sidebarWidth?: number;
	inspectorWidth?: number;
	isPreviewActive?: boolean;
}

export const TopBar = memo(function TopBar({ sidebarWidth = 0, inspectorWidth = 0, isPreviewActive = false }: TopBarProps) {
	const containerRef = useOverflowCalculation();

	return (
		<div
			className="absolute top-0 z-[60] h-12"
			data-tauri-drag-region
			style={{
				left: sidebarWidth,
				right: inspectorWidth,
			}}
		>
			<div
				ref={containerRef}
				className="relative flex items-center h-full px-3 gap-3 overflow-hidden"
				data-tauri-drag-region
			>
				<TopBarSection position="left" />
				<TopBarSection position="center" />
				<TopBarSection position="right" />
			</div>
		</div>
	);
});