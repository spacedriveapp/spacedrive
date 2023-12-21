import clsx from 'clsx';
import { MacTrafficLights } from '~/components/TrafficLights';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { usePlatform } from '~/util/Platform';

import { macOnly } from '../helpers';

export default () => {
	const { platform } = usePlatform();
	const os = useOperatingSystem();

	const showControls = window.location.search.includes('showControls');

	if ((platform === 'tauri' && os == 'macOS') || showControls) {
		return (
			<div data-tauri-drag-region className={clsx('shrink-0', macOnly(os, 'h-7'))}>
				{/* We do not provide the onClick handlers for 'MacTrafficLights' because this is only used in demo mode */}
				{showControls && (
					<MacTrafficLights className="absolute left-[13px] top-[13px] z-50" />
				)}
			</div>
		);
	}

	return <div />;
};
