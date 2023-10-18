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
import { AddLocationButton } from './settings/library/locations/AddLocationButton';
import { TOP_BAR_HEIGHT } from './TopBar';
import { TopBarPortal } from './TopBar/Portal';
import TopBarButton from './TopBar/TopBarButton';

const items: { icon: keyof typeof iconNames; name: string }[] = [
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
		<Dialog.Root
			open={true}
			onOpenChange={(open) => (getDismissibleNoticeStore().ephemeral = !open)}
		>
			<Dialog.Portal>
				<Dialog.Overlay
					className={clsx(
						'fixed inset-0 z-50 bg-app/80 backdrop-blur-sm',
						'radix-state-closed:animate-out radix-state-closed:fade-out-0 radix-state-open:animate-in radix-state-open:fade-in-0'
					)}
					onContextMenu={(e) => e.preventDefault()}
				/>

				<Dialog.Content
					className="fixed left-[50%] top-[50%] z-50 w-96 translate-x-[-50%] translate-y-[-50%] rounded-md border border-app-line bg-app p-2 shadow-lg outline-none duration-200 radix-state-closed:animate-out radix-state-closed:fade-out-0 radix-state-closed:zoom-out-95 radix-state-closed:slide-out-to-left-1/2 radix-state-closed:slide-out-to-top-[48%] radix-state-open:animate-in radix-state-open:fade-in-0 radix-state-open:zoom-in-95 radix-state-open:slide-in-from-left-1/2 radix-state-open:slide-in-from-top-[48%]"
					onContextMenu={(e) => e.preventDefault()}
				>
					<div className="relative flex aspect-video overflow-hidden rounded border border-app-line bg-app-darkBox pl-5 pt-5">
						<div
							className={clsx(
								'relative flex flex-1 flex-col overflow-hidden rounded-tl-lg border-l border-t border-app-line/75 bg-app shadow-lg',
								isDark ? 'shadow-app-shade/40' : 'shadow-app-shade/20'
							)}
						>
							<div className="absolute inset-0 z-50 bg-app/80 backdrop-blur-[2px]" />

							<div
								style={{ height: TOP_BAR_HEIGHT }}
								className="flex items-center gap-3.5 border-b border-sidebar-divider bg-app/90 px-3.5"
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
									{items.map((item) => (
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
					</div>

					<div className="mt-3 text-center">
						<h2 className="font-semibold text-ink">Local Locations</h2>
						<p className="text-sm text-ink-dull">
							Browse your files and folders directly from your device.
						</p>
					</div>

					<div className="mt-3 flex items-center rounded-md border border-accent/25 bg-accent/10 px-3 py-2 text-sm text-ink-dull">
						<Info size={36} className="mr-3 text-accent" />
						<p className="text-ink">
							Consider indexing your local locations for a faster and more efficient
							exploration.
						</p>
					</div>

					<Button
						variant="accent"
						className="mt-2 w-full !rounded"
						size="md"
						onClick={dismiss}
					>
						Got it
					</Button>
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
				withHiddenFiles: false,
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
			if (
				!settingsSnapshot.showHiddenFiles &&
				item.type === 'NonIndexedPath' &&
				item.item.hidden
			)
				continue;

			if (settingsSnapshot.layoutMode !== 'media') ret.push(item);
			else {
				const { kind } = getExplorerItemData(item);

				if (kind === 'Video' || kind === 'Image') ret.push(item);
			}
		}

		return ret;
	}, [query.data, settingsSnapshot.layoutMode, settingsSnapshot.showHiddenFiles]);

	const explorer = useExplorer({
		items,
		settings: explorerSettings
	});

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
			<Explorer />
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
