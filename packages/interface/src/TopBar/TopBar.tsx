import { useEffect, useRef, memo } from "react";
import { useTopBar } from "./Context";
import clsx from "clsx";

interface TopBarProps {
	sidebarWidth?: number;
	inspectorWidth?: number;
	isPreviewActive?: boolean;
}

export const TopBar = memo(function TopBar({ sidebarWidth = 0, inspectorWidth = 0, isPreviewActive = false }: TopBarProps) {
	const { setLeftRef, setCenterRef, setRightRef } = useTopBar();
	const leftRef = useRef<HTMLDivElement>(null);
	const centerRef = useRef<HTMLDivElement>(null);
	const rightRef = useRef<HTMLDivElement>(null);

	useEffect(() => {
		setLeftRef(leftRef);
		setCenterRef(centerRef);
		setRightRef(rightRef);
	}, [setLeftRef, setCenterRef, setRightRef]);

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
				<div ref={centerRef} data-tauri-drag-region className="flex-1 flex items-center justify-center gap-2" />
				<div ref={rightRef} data-tauri-drag-region className="flex items-center gap-2" />

				{/* Right fade mask - hide when preview active */}
				{!isPreviewActive && (
					<div className="absolute right-0 top-0 bottom-0 w-12 bg-gradient-to-l from-app to-transparent pointer-events-none" />
				)}
			</div>
		</div>
	);
});
