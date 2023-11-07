import { ArrowLeft, ArrowRight } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect } from 'react';
import { useNavigate } from 'react-router';
import { Tooltip } from '@sd/ui';
import { useKeyMatcher, useOperatingSystem, useSearchStore, useShortcut } from '~/hooks';

import { useExplorerDroppable } from '../Explorer/View/useExplorerDroppable';
import TopBarButton from './TopBarButton';

export const NavigationButtons = () => {
	const navigate = useNavigate();
	const { isFocused } = useSearchStore();
	const idx = history.state.idx as number;
	const os = useOperatingSystem();
	const { icon } = useKeyMatcher('Meta');

	const droppableBack = useExplorerDroppable({
		navigateTo: -1,
		disabled: isFocused || idx === 0
	});

	const droppableForward = useExplorerDroppable({
		navigateTo: 1,
		disabled: isFocused || idx === history.length - 1
	});

	useShortcut('navBackwardHistory', () => {
		if (idx === 0 || isFocused) return;
		navigate(-1);
	});

	useShortcut('navForwardHistory', () => {
		if (idx === history.length - 1 || isFocused) return;
		navigate(1);
	});

	useEffect(() => {
		if (os === 'windows') return; //windows already navigates back and forth with mouse buttons
		const onMouseDown = (e: MouseEvent) => {
			e.stopPropagation();
			if (e.buttons === 8) {
				if (idx === 0 || isFocused) return;
				navigate(-1);
			} else if (e.buttons === 16) {
				if (idx === history.length - 1 || isFocused) return;
				navigate(1);
			}
		};
		document.addEventListener('mousedown', onMouseDown);
		return () => document.removeEventListener('mousedown', onMouseDown);
	}, [navigate, idx, isFocused, os]);

	return (
		<div data-tauri-drag-region={os === 'macOS'} className="flex">
			<Tooltip keybinds={[icon, '[']} label="Navigate back">
				<TopBarButton
					rounding="left"
					onClick={() => navigate(-1)}
					disabled={isFocused || idx === 0}
					ref={droppableBack.setDroppableRef}
					className={clsx(
						droppableBack.isDroppable && '!bg-app-selected',
						droppableBack.navigateClassName
					)}
				>
					<ArrowLeft size={14} className="m-[4px]" weight="bold" />
				</TopBarButton>
			</Tooltip>
			<Tooltip keybinds={[icon, ']']} label="Navigate forward">
				<TopBarButton
					rounding="right"
					onClick={() => navigate(1)}
					disabled={isFocused || idx === history.length - 1}
					ref={droppableForward.setDroppableRef}
					className={clsx(
						droppableForward.isDroppable && '!bg-app-selected',
						droppableForward.navigateClassName
					)}
				>
					<ArrowRight size={14} className="m-[4px]" weight="bold" />
				</TopBarButton>
			</Tooltip>
		</div>
	);
};
