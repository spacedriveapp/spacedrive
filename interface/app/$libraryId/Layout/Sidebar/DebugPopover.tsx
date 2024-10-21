import { CheckSquare } from '@phosphor-icons/react';
import { useQueryClient } from '@tanstack/react-query';
import { SetStateAction, useContext } from 'react';
import { useNavigate } from 'react-router';
import {
	auth,
	backendFeatures,
	features,
	toggleFeatureFlag,
	useBridgeMutation,
	useBridgeQuery,
	useDebugState,
	useFeatureFlags,
	useLibraryMutation
} from '@sd/client';
import {
	Button,
	Dropdown,
	DropdownMenu,
	Popover,
	Select,
	SelectOption,
	Switch,
	usePopover
} from '@sd/ui';
import { toggleRenderRects } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import {
	explorerOperatingSystemStore,
	useExplorerOperatingSystem
} from '../../Explorer/useExplorerOperatingSystem';
import Setting from '../../settings/Setting';
import { SidebarContext, useSidebarContext } from './SidebarLayout/Context';

export default () => {
	const buildInfo = useBridgeQuery(['buildInfo']);
	const nodeState = useBridgeQuery(['nodeState']);

	const debugState = useDebugState();
	const platform = usePlatform();
	const navigate = useNavigate();

	const sidebar = useContext(SidebarContext);

	const popover = usePopover();

	function handleOpenChange(action: SetStateAction<boolean>) {
		const open = typeof action === 'boolean' ? action : !popover.open;
		popover.setOpen(open);
		sidebar?.onLockedChange(open);
	}

	return (
		<Popover
			popover={{ ...popover, setOpen: handleOpenChange }}
			className="z-[100] p-4 focus:outline-none"
			trigger={
				<h1 className="ml-1 w-full font-plex text-[7pt] tracking-widest text-sidebar-inkFaint/50">
					v{buildInfo.data?.version || '-.-.-'} - {buildInfo.data?.commit || 'dev'}
				</h1>
			}
		>
			<div className="no-scrollbar block h-96 w-[430px] overflow-y-scroll pb-4">
				{/* <Setting mini title="Cloud Origin" description="Change the cloud origin to use">
					<CloudOriginSelect />
				</Setting> */}

				<Setting
					mini
					title="rspc Logger"
					description="Enable the RSPC logger so you can see what's going on in the browser logs."
				>
					<Switch
						checked={debugState.rspcLogger}
						onClick={() => (debugState.rspcLogger = !debugState.rspcLogger)}
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
								debugState.telemetryLogging = false;
							debugState.shareFullTelemetry = !debugState.shareFullTelemetry;
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
								debugState.shareFullTelemetry = true;
							debugState.telemetryLogging = !debugState.telemetryLogging;
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
				{platform.reloadWebview && (
					<Setting mini title="Reload webview" description="Reload the window's webview">
						<div className="mt-2">
							<Button
								size="sm"
								variant="gray"
								onClick={() => {
									if (platform.reloadWebview) platform.reloadWebview();
								}}
							>
								Reload
							</Button>
						</div>
					</Setting>
				)}
				<Setting
					mini
					title="React Query Devtools"
					description="Configure the React Query devtools."
				>
					<Switch
						checked={debugState.reactQueryDevtools}
						onClick={() =>
							(debugState.reactQueryDevtools = !debugState.reactQueryDevtools)
						}
					/>
				</Setting>
				<Setting
					mini
					title="Explorer behavior"
					description="Change the explorer selection behavior"
				>
					<ExplorerBehaviorSelect />
				</Setting>
				{/* <FeatureFlagSelector /> */}
				<InvalidateDebugPanel />
				{/* <TestNotifications /> */}
				<div className="flex gap-2">
					<Button size="sm" variant="gray" onClick={() => navigate('./debug/cache')}>
						Cache Debug
					</Button>
					<Button size="sm" variant="gray" onClick={() => toggleRenderRects()}>
						Toggle DND Rects
					</Button>
				</div>

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

// function TestNotifications() {
// 	const coreNotif = useBridgeMutation(['notifications.test']);
// 	const libraryNotif = useLibraryMutation(['notifications.testLibrary']);

// 	return (
// 		<Setting mini title="Notifications" description="Test the notification system">
// 			<Button onClick={() => coreNotif.mutate(undefined)}>Core</Button>
// 			<Button onClick={() => libraryNotif.mutate(null)}>Library</Button>
// 		</Setting>
// 	);
// }

// function CloudOriginSelect() {
// 	const origin = useBridgeQuery(['cloud.getApiOrigin']);
// 	const setOrigin = useBridgeMutation(['cloud.setApiOrigin']);

// 	const queryClient = useQueryClient();

// 	return (
// 		<>
// 			{origin.data && (
// 				<Select
// 					onChange={(v) =>
// 						setOrigin.mutateAsync(v).then(() => {
// 							auth.logout();
// 							queryClient.invalidateQueries();
// 						})
// 					}
// 					value={origin.data}
// 				>
// 					<SelectOption value="https://api.spacedrive.com">
// 						https://api.spacedrive.com
// 					</SelectOption>
// 					<SelectOption value="http://localhost:3000">http://localhost:3000</SelectOption>
// 				</Select>
// 			)}
// 		</>
// 	);
// }

function ExplorerBehaviorSelect() {
	const { explorerOperatingSystem } = useExplorerOperatingSystem();

	return (
		<Select
			value={explorerOperatingSystem}
			onChange={(v) => (explorerOperatingSystemStore.os = v)}
		>
			<SelectOption value="macOS">macOS</SelectOption>
			<SelectOption value="windows">windows</SelectOption>
		</Select>
	);
}
