import { Gear } from '@phosphor-icons/react';
import { inferSubscriptionResult } from '@spacedrive/rspc-client';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import {
	LibraryContextProvider,
	Procedures,
	useClientContext,
	useDebugState,
	useLibrarySubscription
} from '@sd/client';
import { Button, ButtonLink, Loader, Tooltip } from '@sd/ui';
import { useKeysMatcher, useLocale, useShortcut } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import DebugPopover from '../DebugPopover';
import { FeedbackPopover } from './FeedbackPopover';
import { JobManagerPopover } from './JobManagerPopover';

export default () => {
	const { library } = useClientContext();
	const { t } = useLocale();
	const debugState = useDebugState();
	const navigate = useNavigate();
	const symbols = useKeysMatcher(['Meta', 'Shift']);

	// useShortcut('navToSettings', (e) => {
	// 	e.stopPropagation();
	// 	navigate('settings/client/general');
	// });

	const updater = usePlatform().updater;
	const updaterState = updater?.useSnapshot();

	return (
		<div className="space-y-2">
			{updater && updaterState?.status === 'updateAvailable' && (
				<Button variant="outline" className="w-full" onClick={updater.installUpdate}>
					{t('install_update')}
				</Button>
			)}

			{library && (
				<LibraryContextProvider library={library}>
					<SyncStatusIndicator />
				</LibraryContextProvider>
			)}

			<div className="flex w-full items-center justify-between">
				<div className="flex">
					<ButtonLink
						to="settings/client/general"
						size="icon"
						variant="subtle"
						className="text-sidebar-inkFaint ring-offset-sidebar"
					>
						<Tooltip
							position="top"
							label={t('settings')}
							keybinds={[symbols.Shift.icon, symbols.Meta.icon, 'T']}
						>
							<Gear className="size-5" />
						</Tooltip>
					</ButtonLink>
					<JobManagerPopover />
				</div>

				<FeedbackPopover />
			</div>

			{debugState.enabled && <DebugPopover />}
		</div>
	);
};

function SyncStatusIndicator() {
	const [status, setStatus] = useState<inferSubscriptionResult<Procedures, 'sync.active'>>();

	useLibrarySubscription(['sync.active'], {
		onData: setStatus
	});

	return (
		<div className="flex flex-row items-center gap-1">
			{status?.cloud_ingest && <Loader className="size-5" color="red" />}
			{status?.cloud_send && <Loader className="size-5" color="green" />}
			{status?.cloud_receive && <Loader className="size-5" color="blue" />}
			{status?.ingest && <Loader className="size-5" color="yellow" />}
		</div>
	);
}
