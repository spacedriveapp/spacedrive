import { SiCheckmarx } from '@icons-pack/react-simple-icons';
import {
	features,
	getDebugState,
	isEnabled,
	toggleFeatureFlag,
	useBridgeMutation,
	useBridgeQuery,
	useDebugState,
	useFeatureFlags,
	useLibraryMutation
} from '@sd/client';
import { Button, Dropdown, DropdownMenu, Popover, Select, SelectOption, Switch } from '@sd/ui';
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
			trigger={
				<h1 className="ml-1 w-full text-[7pt] text-sidebar-inkFaint/50">
					v{buildInfo.data?.version || '-.-.-'} - {buildInfo.data?.commit || 'dev'}
				</h1>
			}
		>
			<div className="no-scrollbar block h-96 w-[430px] overflow-y-scroll pb-4">
				<Setting
					mini
					title="rspc Logger"
					description="Enable the RSPC logger so you can see what's going on in the browser logs."
				>
					<Switch
						checked={debugState.rspcLogger}
						onClick={() => (getDebugState().rspcLogger = !debugState.rspcLogger)}
					/>
				</Setting>
				<Setting
					mini
					title="Share telemetry"
					description="Share telemetry, even in debug mode (telemetry sharing must also be enabled in your client settings)"
				>
					<Switch
						checked={debugState.shareFullTelemetry}
						onClick={() => {
							// if debug telemetry sharing is about to be disabled, but telemetry logging is enabled
							// then disable it
							if (
								!debugState.shareFullTelemetry === false &&
								debugState.telemetryLogging
							)
								getDebugState().telemetryLogging = false;
							getDebugState().shareFullTelemetry = !debugState.shareFullTelemetry;
						}}
					/>
				</Setting>
				<Setting
					mini
					title="Telemetry logger"
					description="Enable the telemetry logger so you can see what's going on in the browser logs"
				>
					<Switch
						checked={debugState.telemetryLogging}
						onClick={() => {
							// if telemetry logging is about to be enabled, but debug telemetry sharing is disabled
							// then enable it
							if (
								!debugState.telemetryLogging &&
								debugState.shareFullTelemetry === false
							)
								getDebugState().shareFullTelemetry = true;
							getDebugState().telemetryLogging = !debugState.telemetryLogging;
						}}
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
									if (nodeState?.data?.data_path)
										platform.openPath!(nodeState?.data?.data_path);
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
				<FeatureFlagSelector />
				<InvalidateDebugPanel />
				<TestNotifications />

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

function InvalidateDebugPanel() {
	const { data: count } = useBridgeQuery(['invalidation.test-invalidate']);
	const { mutate } = useLibraryMutation(['invalidation.test-invalidate-mutation']);

	return (
		<Setting
			mini
			title="Invalidate Debug Panel"
			description={`Pressing the button issues an invalidate to the query rendering this number: ${count}`}
		>
			<div className="mt-2">
				<Button size="sm" variant="gray" onClick={() => mutate(null)}>
					Invalidate
				</Button>
			</div>
		</Setting>
	);
}

function FeatureFlagSelector() {
	useFeatureFlags(); // Subscribe to changes

	return (
		<DropdownMenu.Root
			trigger={
				<Dropdown.Button variant="gray">
					<span className="truncate">Feature Flags</span>
				</Dropdown.Button>
			}
			className="mt-1 shadow-none data-[side=bottom]:slide-in-from-top-2 dark:divide-menu-selected/30 dark:border-sidebar-line dark:bg-sidebar-box"
			alignToTrigger
		>
			{features.map((feat) => (
				<div key={feat} className="flex text-white">
					{isEnabled(feat) && <SiCheckmarx />}

					<DropdownMenu.Item
						label={feat}
						iconProps={{ weight: 'bold', size: 16 }}
						onClick={() => toggleFeatureFlag(feat)}
						className="font-medium"
					/>
				</div>
			))}
		</DropdownMenu.Root>
	);
}

function TestNotifications() {
	const coreNotif = useBridgeMutation(['notifications.test']);
	const libraryNotif = useLibraryMutation(['notifications.testLibrary']);

	return (
		<Setting mini title="Notifications" description="Test the notification system">
			<Button onClick={() => coreNotif.mutate(undefined)}>Core</Button>
			<Button onClick={() => libraryNotif.mutate(null)}>Library</Button>
		</Setting>
	);
}
