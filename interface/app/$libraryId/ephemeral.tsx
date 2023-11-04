import { ArrowLeft, ArrowRight, Info } from '@phosphor-icons/react';
import * as Dialog from '@radix-ui/react-dialog';
import { iconNames } from '@sd/assets/util';
import clsx from 'clsx';
import { memo, Suspense, useDeferredValue, useMemo } from 'react';
import {
	ExplorerItem,
	getExplorerItemData,
	useLibraryQuery,
	type EphemeralPathOrder
} from '@sd/client';
import { Button, Tooltip } from '@sd/ui';
import { PathParamsSchema, type PathParams } from '~/app/route-schemas';
import { Icon } from '~/components';
import {
	getDismissibleNoticeStore,
	useDismissibleNoticeStore,
	useIsDark,
	useKeyDeleteFile,
	useOperatingSystem,
	useZodSearchParams
} from '~/hooks';

import Explorer from './Explorer';
import { ExplorerContextProvider } from './Explorer/Context';
import {
	createDefaultExplorerSettings,
	getExplorerStore,
	nonIndexedPathOrderingSchema
} from './Explorer/store';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from './Explorer/useExplorer';
import { EmptyNotice } from './Explorer/View';
import { AddLocationButton } from './settings/library/locations/AddLocationButton';
import { TOP_BAR_HEIGHT } from './TopBar';
import { TopBarPortal } from './TopBar/Portal';
import TopBarButton from './TopBar/TopBarButton';

const NOTICE_ITEMS: { icon: keyof typeof iconNames; name: string }[] = [
	{
		icon: 'Folder',
		name: 'Documents'
	},
	{
		icon: 'Archive',
		name: 'Keep-Safe'
	},
	{
		icon: 'Executable',
		name: 'Spacedrive'
	},
	{
		icon: 'Folder',
		name: 'Music'
	}
];

const EphemeralNotice = ({ path }: { path: string }) => {
	const isDark = useIsDark();
	const { ephemeral: dismissed } = useDismissibleNoticeStore();

	const dismiss = () => (getDismissibleNoticeStore().ephemeral = true);

	return (
		<Dialog.Root open={!dismissed}>
			<Dialog.Portal>
				<Dialog.Overlay className="fixed inset-0 z-50 bg-app/80 backdrop-blur-sm radix-state-closed:animate-out radix-state-closed:fade-out-0 radix-state-open:animate-in radix-state-open:fade-in-0" />
				<Dialog.Content className="fixed left-[50%] top-[50%] z-50 w-96 translate-x-[-50%] translate-y-[-50%] overflow-hidden rounded-md border border-app-line bg-app shadow-lg outline-none duration-200 radix-state-closed:animate-out radix-state-closed:fade-out-0 radix-state-closed:zoom-out-95 radix-state-closed:slide-out-to-left-1/2 radix-state-closed:slide-out-to-top-[48%] radix-state-open:animate-in radix-state-open:fade-in-0 radix-state-open:zoom-in-95 radix-state-open:slide-in-from-left-1/2 radix-state-open:slide-in-from-top-[48%]">
					<div className="relative flex aspect-video overflow-hidden border-b border-app-line/50 bg-gradient-to-b from-app-darkBox to-app to-80% pl-5 pt-5">
						<div
							className={clsx(
								'relative flex flex-1 flex-col overflow-hidden rounded-tl-lg border-l border-t border-app-line/75 bg-app shadow-lg',
								isDark ? 'shadow-app-shade/40' : 'shadow-app-shade/20'
							)}
						>
							<div className="absolute inset-0 z-50 bg-app/80 backdrop-blur-[2px]" />

							<div
								style={{ height: TOP_BAR_HEIGHT }}
								className="flex items-center gap-3.5 border-b border-sidebar-divider px-3.5"
							>
								<div className="flex">
									<TopBarButton rounding="left">
										<ArrowLeft size={14} className="m-[4px]" weight="bold" />
									</TopBarButton>

									<TopBarButton rounding="right" disabled>
										<ArrowRight size={14} className="m-[4px]" weight="bold" />
									</TopBarButton>
								</div>

								<Tooltip
									label="Add path as an indexed location"
									className="z-50 w-max min-w-0 shrink animate-pulse [animation-duration:_3000ms] hover:animate-none"
								>
									<AddLocationButton
										path={path}
										className="shadow-md"
										onClick={dismiss}
									/>
								</Tooltip>
							</div>

							<div className="relative flex-1">
								<div className="absolute inset-0 grid w-[115%] grid-cols-4 gap-3 pl-3 pt-3">
									{NOTICE_ITEMS.map((item) => (
										<div key={item.name} className="flex flex-col items-center">
											<Icon name={item.icon} draggable={false} />
											<span className="text-center text-xs font-medium text-ink">
												{item.name}
											</span>
										</div>
									))}
								</div>
							</div>
						</div>

						<div className="absolute inset-x-0 bottom-0 z-50 h-4 bg-gradient-to-t from-app/70 to-transparent" />
					</div>

					<div className="p-3 pt-0">
						<div className="py-4 text-center">
							<h2 className="text-lg font-semibold text-ink">Local Locations</h2>
							<p className="mt-px text-sm text-ink-dull">
								Browse your files and folders directly from your device.
							</p>
						</div>

						<div className="flex items-center rounded-md border border-app-line bg-app-box px-3 py-2 text-ink-faint">
							<Info size={20} weight="light" className="mr-2.5 shrink-0" />
							<p className="text-sm font-light">
								Consider indexing your local locations for a faster and more
								efficient exploration.
							</p>
						</div>

						<Button
							variant="accent"
							className="mt-3 w-full !rounded"
							size="md"
							onClick={dismiss}
						>
							Got it
						</Button>
					</div>
				</Dialog.Content>
			</Dialog.Portal>
		</Dialog.Root>
	);
};

const EphemeralExplorer = memo((props: { args: PathParams }) => {
	const os = useOperatingSystem();
	const { path } = props.args;

	const explorerSettings = useExplorerSettings({
		settings: useMemo(
			() =>
				createDefaultExplorerSettings<EphemeralPathOrder>({
					order: {
						field: 'name',
						value: 'Asc'
					}
				}),
			[]
		),
		orderingKeys: nonIndexedPathOrderingSchema
	});

	const settingsSnapshot = explorerSettings.useSettingsSnapshot();

	const query = useLibraryQuery(
		[
			'search.ephemeralPaths',
			{
				path: path ?? (os === 'windows' ? 'C:\\' : '/'),
				withHiddenFiles: settingsSnapshot.showHiddenFiles,
				order: settingsSnapshot.order
			}
		],
		{
			enabled: path != null,
			suspense: true,
			onSuccess: () => getExplorerStore().resetNewThumbnails()
		}
	);

	const items = useMemo(() => {
		if (!query.data) return [];

		const ret: ExplorerItem[] = [];

		for (const item of query.data.entries) {
			if (settingsSnapshot.layoutMode !== 'media') ret.push(item);
			else {
				const { kind } = getExplorerItemData(item);

				if (kind === 'Video' || kind === 'Image') ret.push(item);
			}
		}

		return ret;
	}, [query.data, settingsSnapshot.layoutMode]);

	const explorer = useExplorer({
		items,
		parent: path != null ? { type: 'Ephemeral', path } : undefined,
		settings: explorerSettings,
		layouts: { media: false }
	});

	useKeyDeleteFile(explorer.selectedItems, null);

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal
				left={
					<Tooltip
						label="Add path as an indexed location"
						className="w-max min-w-0 shrink"
					>
						<AddLocationButton path={path} />
					</Tooltip>
				}
				right={<DefaultTopBarOptions />}
				noSearch={true}
			/>
			<Explorer
				emptyNotice={
					<EmptyNotice
						loading={query.isFetching}
						icon={<Icon name="FolderNoSpace" size={128} />}
						message="No files found here"
					/>
				}
			/>
		</ExplorerContextProvider>
	);
});

export const Component = () => {
	const [pathParams] = useZodSearchParams(PathParamsSchema);

	const path = useDeferredValue(pathParams);

	return (
		<Suspense>
			<EphemeralNotice path={path.path ?? ''} />
			<EphemeralExplorer args={path} />
		</Suspense>
	);
};
