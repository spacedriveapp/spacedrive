import { ArrowLeft, ArrowRight } from '@phosphor-icons/react';
import { useEffect } from 'react';
import { useNavigate } from 'react-router';
import { Tooltip } from '@sd/ui';
import { useKeybind, useKeyMatcher, useOperatingSystem, useSearchStore } from '~/hooks';

import TopBarButton from './TopBarButton';

export const NavigationButtons = () => {
	const navigate = useNavigate();
	const { isFocused } = useSearchStore();
	const os = useOperatingSystem();
	const idx = history.state.idx as number;
	const { icon, key } = useKeyMatcher('Meta');

	useKeybind([key, '['], () => {
		if (idx === 0 || isFocused) return;
		navigate(-1);
	});
	useKeybind([key, ']'], () => {
		if (idx === history.length - 1 || isFocused) return;
		navigate(1);
	});

	useEffect(() => {
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
		window.addEventListener('mousedown', onMouseDown);
		return () => window.removeEventListener('mousedown', onMouseDown);
	}, [navigate, idx, isFocused]);

	return (
		<div data-tauri-drag-region={os === 'macOS'} className="flex">
			<Tooltip keybinds={[icon, '[']} label="Navigate back">
				<TopBarButton
					rounding="left"
					// className="text-[14px] text-ink-dull"
					onClick={() => navigate(-1)}
					disabled={isFocused || idx === 0}
				>
					<ArrowLeft size={14} className="m-[4px]" weight="bold" />
				</TopBarButton>
			</Tooltip>
			<Tooltip keybinds={[icon, ']']} label="Navigate forward">
				<TopBarButton
					rounding="right"
					// className="text-[14px] text-ink-dull"
					onClick={() => navigate(1)}
					disabled={isFocused || idx === history.length - 1}
				>
					<ArrowRight size={14} className="m-[4px]" weight="bold" />
				</TopBarButton>
			</Tooltip>
		</div>
	);
};
