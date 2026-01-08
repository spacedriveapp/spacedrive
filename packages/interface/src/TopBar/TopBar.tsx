import { useEffect, useRef, memo } from "react";
import { useTopBar } from "./Context";

interface TopBarProps {
	sidebarWidth?: number;
	inspectorWidth?: number;
	isPreviewActive?: boolean;
}

export const TopBar = memo(function TopBar({ sidebarWidth = 0, inspectorWidth = 0, isPreviewActive = false }: TopBarProps) {
	const { setLeftRef, setRightRef } = useTopBar();
	const leftRef = useRef<HTMLDivElement>(null);
	const rightRef = useRef<HTMLDivElement>(null);

	useEffect(() => {
		setLeftRef(leftRef);
		setRightRef(rightRef);
	}, [setLeftRef, setRightRef]);

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
				className="relative flex items-center h-full px-3 gap-3 overflow-hidden"
				data-tauri-drag-region
			>
				<div ref={leftRef} data-tauri-drag-region className="flex items-center gap-2" />
				<div className="flex-1" />
				<div ref={rightRef} data-tauri-drag-region className="flex items-center gap-2" />
			</div>
		</div>
	);
});