import { getDebugState, useBridgeQuery, useDebugState } from '@sd/client';
import { Button, Popover, Select, SelectOption, Switch } from '@sd/ui';
import { usePlatform } from '~/util/Platform';
import Setting from '../../settings/Setting';

export default () => {
	const buildInfo = useBridgeQuery(['buildInfo']);
	const nodeState = useBridgeQuery(['nodeState']);

	const debugState = useDebugState();
	const platform = usePlatform();

	return (
		<Popover
			className="p-4 focus:outline-none"
			transformOrigin="bottom left"
			trigger={
				<h1 className="text-ink-faint/50 ml-1 mt-1 w-full text-[7pt]">
					v{buildInfo.data?.version || '-.-.-'} - {buildInfo.data?.commit || 'dev'}
				</h1>
			}
		>
			<div className="block h-96 w-[430px]">
				<Setting
					mini
					title="rspc Logger"
					description="Enable the logger link so you can see what's going on in the browser logs."
				>
					<Switch
						checked={debugState.rspcLogger}
						onClick={() => (getDebugState().rspcLogger = !debugState.rspcLogger)}
					/>
				</Setting>
				{platform.openPath && (
					<Setting
						mini
						title="Open Data Directory"
						description="Quickly get to your Spacedrive database"
					>
						<div className="mt-2">
							<Button
								size="sm"
								variant="gray"
								onClick={() => {
									if (nodeState?.data?.data_path) platform.openPath!(nodeState?.data?.data_path);
								}}
							>
								Open
							</Button>
						</div>
					</Setting>
				)}
				<Setting
					mini
					title="React Query Devtools"
					description="Configure the React Query devtools."
				>
					<Select
						value={debugState.reactQueryDevtools}
						size="sm"
						onChange={(value) => (getDebugState().reactQueryDevtools = value as any)}
					>
						<SelectOption value="disabled">Disabled</SelectOption>
						<SelectOption value="invisible">Invisible</SelectOption>
						<SelectOption value="enabled">Enabled</SelectOption>
					</Select>
				</Setting>

				{/* {platform.showDevtools && (
					<SettingContainer
						mini
						title="Devtools"
						description="Allow opening browser devtools in a production build"
					>
						<div className="mt-2">
							<Button size="sm" variant="gray" onClick={platform.showDevtools}>
								Show
							</Button>
						</div>
					</SettingContainer>
				)} */}
			</div>
		</Popover>
	);
};
