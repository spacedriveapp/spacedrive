import { inferSubscriptionResult } from '@oscartbeaumont-sd/rspc-client';
import { Gear } from '@phosphor-icons/react';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import {
	JobManagerContextProvider,
	LibraryContextProvider,
	Procedures,
	useClientContext,
	useDebugState,
	useLibrarySubscription,
	useUnsafeStreamedQuery
} from '@sd/client';
import { Button, ButtonLink, Popover, Tooltip, usePopover } from '@sd/ui';
import { useKeysMatcher, useLocale, useShortcut } from '~/hooks';
import { useRoutingContext } from '~/RoutingContext';
import { usePlatform } from '~/util/Platform';

import DebugPopover from '../DebugPopover';
import { IsRunningJob, JobManager } from '../JobManager';
import { useSidebarStore } from '../store';
import FeedbackButton from './FeedbackButton';

export default () => {
	const { library } = useClientContext();
	const { visible } = useRoutingContext();
	const { t } = useLocale();
	const debugState = useDebugState();
	const navigate = useNavigate();
	const symbols = useKeysMatcher(['Meta', 'Shift']);
	const store = useSidebarStore();

	useShortcut('navToSettings', (e) => {
		e.stopPropagation();
		navigate('settings/client/general');
	});

	const updater = usePlatform().updater;
	const updaterState = updater?.useSnapshot();

	const jobManagerPopover = usePopover();

	useShortcut('toggleJobManager', () => jobManagerPopover.setOpen((open) => !open));

	return (
		<div className="space-y-2">
			{updater && updaterState && (
				<>
					{updaterState.status === 'updateAvailable' && (
						<Button
							variant="outline"
							className="w-full"
							onClick={updater.installUpdate}
						>
							{t('install_update')}
						</Button>
					)}
				</>
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
					<JobManagerContextProvider>
						<Popover
							popover={{
								...jobManagerPopover,
								open: jobManagerPopover.open || (store.pinJobManager && visible)
							}}
							trigger={
								<Button
									id="job-manager-button"
									size="icon"
									variant="subtle"
									className="text-sidebar-inkFaint ring-offset-sidebar radix-state-open:bg-sidebar-selected/50"
									disabled={!library}
								>
									{library && (
										<Tooltip
											label={t('recent_jobs')}
											position="top"
											keybinds={[symbols.Meta.icon, 'J']}
										>
											<IsRunningJob />
										</Tooltip>
									)}
								</Button>
							}
						>
							<div className="block h-96 w-[430px]">
								<JobManager />
							</div>
						</Popover>
					</JobManagerContextProvider>
				</div>
				<FeedbackButton />
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

	return null;
}
