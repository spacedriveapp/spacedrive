import {memo} from 'react';
import {usePlatform} from '../contexts/PlatformContext';
import {TopBarSection} from './Section';
import {useOverflowCalculation} from './useOverflowCalculation';

interface TopBarProps {
	sidebarWidth?: number;
	inspectorWidth?: number;
	reserveWindowControls?: boolean;
}

// Traffic lights on macOS are ~80px from left edge when sidebar is collapsed
const MACOS_TRAFFIC_LIGHT_WIDTH = 90;

// Detect macOS once
const isMacOS =
	typeof navigator !== 'undefined' &&
	(navigator.platform.toLowerCase().includes('mac') ||
		navigator.userAgent.includes('Mac'));

export const TopBar = memo(function TopBar({
	sidebarWidth = 0,
	inspectorWidth = 0,
	reserveWindowControls = false
}: TopBarProps) {
	const containerRef = useOverflowCalculation();
	const platform = usePlatform();
	const leftPadding =
		platform.platform === 'tauri' && isMacOS && reserveWindowControls
			? MACOS_TRAFFIC_LIGHT_WIDTH
			: 0;

	return (
		<div
			className="absolute top-0 z-[60] h-12"
			data-tauri-drag-region
			style={{
				left: sidebarWidth,
				right: inspectorWidth
			}}
		>
			<div
				ref={containerRef}
				className="relative flex h-full items-center gap-3 overflow-hidden px-3"
				data-tauri-drag-region
				style={{
					paddingLeft: leftPadding ? `${leftPadding}px` : undefined
				}}
			>
				<TopBarSection position="left" />
				<TopBarSection position="center" />
				<TopBarSection position="right" />
			</div>
		</div>
	);
});
