import { ArrowLeft, ArrowRight } from '@phosphor-icons/react';
import { useEffect } from 'react';
import { useNavigate } from 'react-router';
import { Tooltip } from '@sd/ui';
import { useKeyMatcher, useOperatingSystem, useShortcut } from '~/hooks';
import { useRoutingContext } from '~/RoutingContext';

import TopBarButton from './TopBarButton';

export const NavigationButtons = () => {
	const { currentIndex, maxIndex } = useRoutingContext();

	const navigate = useNavigate();
	const os = useOperatingSystem();
	const { icon } = useKeyMatcher('Meta');

	const canGoBack = currentIndex !== 0;
	const canGoForward = currentIndex !== maxIndex;

	useShortcut('navBackwardHistory', () => {
		if (!canGoBack) return;
		navigate(-1);
	});

	useShortcut('navForwardHistory', () => {
		if (!canGoForward) return;
		navigate(1);
	});

	useEffect(() => {
		if (os === 'windows') return; //windows already navigates back and forth with mouse buttons

		const onMouseDown = (e: MouseEvent) => {
			e.stopPropagation();
			if (e.buttons === 8) {
				if (!canGoBack) return;
				navigate(-1);
			} else if (e.buttons === 16) {
				if (!canGoForward) return;
				navigate(1);
			}
		};
		document.addEventListener('mousedown', onMouseDown);
		return () => document.removeEventListener('mousedown', onMouseDown);
	}, [navigate, os, canGoBack, canGoForward]);

	return (
		<div data-tauri-drag-region={os === 'macOS'} className="flex">
			<Tooltip keybinds={[icon, '[']} label="Navigate back">
				<TopBarButton
					rounding="left"
					// className="text-[14px] text-ink-dull"
					onClick={() => navigate(-1)}
					disabled={!canGoBack}
				>
					<ArrowLeft size={14} className="m-[4px]" weight="bold" />
				</TopBarButton>
			</Tooltip>
			<Tooltip keybinds={[icon, ']']} label="Navigate forward">
				<TopBarButton
					rounding="right"
					// className="text-[14px] text-ink-dull"
					onClick={() => navigate(1)}
					disabled={!canGoForward}
				>
					<ArrowRight size={14} className="m-[4px]" weight="bold" />
				</TopBarButton>
			</Tooltip>
		</div>
	);
};
