import { SetStateAction } from 'react';
import { JobManagerContextProvider, useClientContext } from '@sd/client';
import { Button, Popover, Tooltip, usePopover } from '@sd/ui';
import { useKeysMatcher, useLocale, useShortcut } from '~/hooks';
import { useRoutingContext } from '~/RoutingContext';

import { IsRunningJob, JobManager } from '../JobManager';
import { useSidebarStore } from '../store';
import { useSidebarContext } from './Context';

export function JobManagerPopover() {
	const { t } = useLocale();
	const { library } = useClientContext();
	const { visible } = useRoutingContext();
	const { Meta } = useKeysMatcher(['Meta', 'Shift']);
	const { pinJobManager } = useSidebarStore();

	const sidebar = useSidebarContext();

	const popover = usePopover();

	function handleOpenChange(action: SetStateAction<boolean>) {
		const open = typeof action === 'boolean' ? action : !popover.open;
		popover.setOpen(open);
		sidebar.onLockedChange(open);
	}

	useShortcut('toggleJobManager', () => {
		const open = !popover.open;

		if (sidebar.collapsed && !sidebar.show && open) {
			sidebar.onLockedChange(true);
			// Wait for the sidebar to open
			setTimeout(() => handleOpenChange(open), 120);
		} else {
			handleOpenChange(open);
		}
	});

	return (
		<JobManagerContextProvider>
			<Popover
				popover={{
					open: popover.open || (pinJobManager && visible),
					setOpen: handleOpenChange
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
								keybinds={[Meta.icon, 'J']}
							>
								<IsRunningJob />
							</Tooltip>
						)}
					</Button>
				}
				className="z-[100]"
				// overlay
			>
				<div className="block h-96 w-[430px]">
					<JobManager />
				</div>
			</Popover>
		</JobManagerContextProvider>
	);
}
