import { ArrowLeft, ArrowRight } from '@phosphor-icons/react';
import { useNavigate } from 'react-router';
import { ModifierKeys, Tooltip } from '@sd/ui';
import { useOperatingSystem, useSearchStore } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';

import TopBarButton from './TopBarButton';

export const NavigationButtons = () => {
	const navigate = useNavigate();
	const { isFocused } = useSearchStore();
	const idx = history.state.idx as number;
	const os = useOperatingSystem();
	const keybind = keybindForOs(os);

	return (
		<div data-tauri-drag-region className="flex">
			<Tooltip keybinds={[keybind([ModifierKeys.Control], ['←'])]} label="Navigate back">
				<TopBarButton
					rounding="left"
					// className="text-[14px] text-ink-dull"
					onClick={() => navigate(-1)}
					disabled={isFocused || idx === 0}
				>
					<ArrowLeft size={14} className="m-[4px]" weight="bold" />
				</TopBarButton>
			</Tooltip>
			<Tooltip keybinds={[keybind([ModifierKeys.Control], ['→'])]} label="Navigate forward">
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
