import { Gear } from 'phosphor-react';
import { useClientContext, useDebugState } from '@sd/client';
import { Button, ButtonLink, Popover, Tooltip } from '@sd/ui';
import DebugPopover from './DebugPopover';
import { IsRunningJob, JobsManager } from './JobManager';

export default () => {
	const { library } = useClientContext();
	const debugState = useDebugState();

	return (
		<div className="space-y-1">
			<div className="flex">
				<ButtonLink
					to="settings/client/general"
					size="icon"
					variant="subtle"
					className="text-ink-faint ring-offset-sidebar"
				>
					<Tooltip label="Settings">
						<Gear className="h-5 w-5" />
					</Tooltip>
				</ButtonLink>
				<Popover
					trigger={
						<Button
							size="icon"
							variant="subtle"
							className="text-ink-faint ring-offset-sidebar radix-state-open:bg-sidebar-selected/50"
							disabled={!library}
						>
							{library && (
								<Tooltip label="Recent Jobs">
									<IsRunningJob />
								</Tooltip>
							)}
						</Button>
					}
				>
					<div className="block h-96 w-[430px]">
						<JobsManager />
					</div>
				</Popover>
			</div>
			{debugState.enabled && <DebugPopover />}
		</div>
	);
};
