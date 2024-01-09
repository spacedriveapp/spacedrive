import { Gear } from '@phosphor-icons/react';
import { useNavigate } from 'react-router';
import { JobManagerContextProvider, useClientContext, useDebugState } from '@sd/client';
import { Button, ButtonLink, Popover, Tooltip, usePopover } from '@sd/ui';
import { useKeysMatcher, useLocale, useShortcut } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import DebugPopover from '../DebugPopover';
import { IsRunningJob, JobManager } from '../JobManager';
import FeedbackButton from './FeedbackButton';

export default () => {
	const { library } = useClientContext();
	const debugState = useDebugState();
	const navigate = useNavigate();
	const symbols = useKeysMatcher(['Meta', 'Shift']);

	const { t } = useLocale();

	useShortcut('navToSettings', (e) => {
		e.stopPropagation();
		navigate('settings/client/general');
	});

	const updater = usePlatform().updater;
	const updaterState = updater?.useSnapshot();

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
							<Gear className="h-5 w-5" />
						</Tooltip>
					</ButtonLink>
					<JobManagerContextProvider>
						<Popover
							popover={usePopover()}
							keybind={[symbols.Meta.key, 'j']}
							trigger={
								<Button
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
