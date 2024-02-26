import { ArrowLeft, ArrowRight } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect } from 'react';
import { useNavigate } from 'react-router';
import { Tooltip } from '@sd/ui';
import { useKeyMatcher, useLocale, useOperatingSystem, useShortcut } from '~/hooks';
import { useRoutingContext } from '~/RoutingContext';

import { useExplorerDroppable } from '../Explorer/useExplorerDroppable';
import TopBarButton from './TopBarButton';

export const NavigationButtons = () => {
	const { currentIndex, maxIndex } = useRoutingContext();

	const { t } = useLocale();

	const navigate = useNavigate();
	const os = useOperatingSystem();
	const { icon } = useKeyMatcher('Meta');

	const canGoBack = currentIndex !== 0;
	const canGoForward = currentIndex !== maxIndex;

	const droppableBack = useExplorerDroppable({
		navigateTo: -1,
		disabled: !canGoBack
	});

	const droppableForward = useExplorerDroppable({
		navigateTo: 1,
		disabled: !canGoForward
	});

	useShortcut('navBackwardHistory', () => {
		if (!canGoBack) return;
		navigate(-1);
	});

	useShortcut('navForwardHistory', () => {
		if (!canGoForward) return;
		navigate(1);
	});

	useEffect(() => {
		const onMouseDown = (e: MouseEvent) => {
			if (os === 'browser') return;
			e.stopPropagation();
			if (e.buttons === 8 || e.buttons === 3) {
				if (!canGoBack) return;
				navigate(-1);
			} else if (e.buttons === 16 || e.buttons === 4) {
				if (!canGoForward) return;
				navigate(1);
			}
		};
		document.addEventListener('mousedown', onMouseDown);
		return () => document.removeEventListener('mousedown', onMouseDown);
	}, [navigate, os, canGoBack, canGoForward]);

	return (
		<div data-tauri-drag-region={os === 'macOS'} className="flex">
			<Tooltip keybinds={[icon, '[']} label={t('navigate_back')}>
				<TopBarButton
					rounding="left"
					onClick={() => navigate(-1)}
					disabled={!canGoBack}
					ref={droppableBack.setDroppableRef}
					className={clsx(
						droppableBack.isDroppable && '!bg-app-selected',
						droppableBack.className
					)}
				>
					<ArrowLeft size={14} className="m-[4px]" weight="bold" />
				</TopBarButton>
			</Tooltip>
			<Tooltip keybinds={[icon, ']']} label={t('navigate_forward')}>
				<TopBarButton
					rounding="right"
					onClick={() => navigate(1)}
					disabled={!canGoForward}
					ref={droppableForward.setDroppableRef}
					className={clsx(
						droppableForward.isDroppable && '!bg-app-selected',
						droppableForward.className
					)}
				>
					<ArrowRight size={14} className="m-[4px]" weight="bold" />
				</TopBarButton>
			</Tooltip>
		</div>
	);
};
