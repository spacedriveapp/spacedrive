import { ArrowLeft, ArrowRight, Info } from '@phosphor-icons/react';
import * as Dialog from '@radix-ui/react-dialog';
import { iconNames } from '@sd/assets/util';
import clsx from 'clsx';
import { memo, Suspense, useDeferredValue, useMemo } from 'react';
import { match } from 'ts-pattern';
import {
	ExplorerItem,
	getExplorerItemData,
	ItemData,
	SortOrder,
	useLibraryContext,
	useUnsafeStreamedQuery
} from '@sd/client';
import { Button, Tooltip } from '@sd/ui';
import { PathParamsSchema, type PathParams } from '~/app/route-schemas';
import { Icon } from '~/components';
import {
	getDismissibleNoticeStore,
	useDismissibleNoticeStore,
	useIsDark,
	useKeyDeleteFile,
	useLocale,
	useOperatingSystem,
	useZodSearchParams
} from '~/hooks';
import { useRouteTitle } from '~/hooks/useRouteTitle';

import Explorer from './Explorer';
import { ExplorerContextProvider } from './Explorer/Context';
import {
	createDefaultExplorerSettings,
	explorerStore,
	nonIndexedPathOrderingSchema
} from './Explorer/store';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from './Explorer/useExplorer';
import { EmptyNotice } from './Explorer/View/EmptyNotice';
import { AddLocationButton } from './settings/library/locations/AddLocationButton';
import { useTopBarContext } from './TopBar/Context';
import { TopBarPortal } from './TopBar/Portal';
import TopBarButton from './TopBar/TopBarButton';

export type EphemeralPathOrder =
	| { field: 'name'; value: SortOrder }
	| { field: 'sizeInBytes'; value: SortOrder }
	| { field: 'dateCreated'; value: SortOrder }
	| { field: 'dateModified'; value: SortOrder };

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
	const { t } = useLocale();

	const isDark = useIsDark();
	const { ephemeral: dismissed } = useDismissibleNoticeStore();

	const topbar = useTopBarContext();

	const dismiss = () => (getDismissibleNoticeStore().ephemeral = true);

	return (
		<Dialog.Root open={!dismissed}>
			<Dialog.Portal>
				<Dialog.Overlay className="fixed inset-0 z-50 bg-app/80 backdrop-blur-sm radix-state-closed:animate-out radix-state-closed:fade-out-0 radix-state-open:animate-in radix-state-open:fade-in-0" />
				<Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-96 -translate-x-1/2 -translate-y-1/2 overflow-hidden rounded-md border border-app-line bg-app shadow-lg outline-none duration-200 radix-state-closed:animate-out radix-state-closed:fade-out-0 radix-state-closed:zoom-out-95 radix-state-closed:slide-out-to-left-1/2 radix-state-closed:slide-out-to-top-[48%] radix-state-open:animate-in radix-state-open:fade-in-0 radix-state-open:zoom-in-95 radix-state-open:slide-in-from-left-1/2 radix-state-open:slide-in-from-top-[48%]">
					<div className="relative flex aspect-video overflow-hidden border-b border-app-line/50 bg-gradient-to-b from-app-darkBox to-app to-80% pl-5 pt-5">
						<div
							className={clsx(
								'relative flex flex-1 flex-col overflow-hidden rounded-tl-lg border-l border-t border-app-line/75 bg-app shadow-lg',
								isDark ? 'shadow-app-shade/40' : 'shadow-app-shade/20'
							)}
						>
							<div className="absolute inset-0 z-50 bg-app/80 backdrop-blur-[2px]" />

							<div
								style={{ height: topbar.topBarHeight }}
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
									label={t('add_location_tooltip')}
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
							<h2 className="text-lg font-semibold text-ink">
								{t('local_locations')}
							</h2>
							<p className="mt-px text-sm text-ink-dull">
								{t('ephemeral_notice_browse')}
							</p>
						</div>

						<div className="flex items-center rounded-md border border-app-line bg-app-box px-3 py-2 text-ink-faint">
							<Info size={20} weight="light" className="mr-2.5 shrink-0" />
							<p className="text-sm font-light">
								{t('ephemeral_notice_consider_indexing')}
							</p>
						</div>

						<Button
							variant="accent"
							className="mt-3 w-full !rounded"
							size="md"
							onClick={dismiss}
						>
							{t('got_it')}
						</Button>
					</div>
				</Dialog.Content>
			</Dialog.Portal>
		</Dialog.Root>
	);
};

const EphemeralExplorer = memo((props: { args: PathParams }) => {
	const { path } = props.args;

	const os = useOperatingSystem();

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

	const libraryCtx = useLibraryContext();
	const query = useUnsafeStreamedQuery(
		[
			'search.ephemeralPaths',
			{
				library_id: libraryCtx.library.uuid,
				arg: {
					from: 'path',
					path: path ?? (os === 'windows' ? 'C:\\' : '/'),
					withHiddenFiles: settingsSnapshot.showHiddenFiles
				}
			}
		],
		{
			enabled: path != null,
			suspense: true,
			onSuccess: () => explorerStore.resetCache(),
			onBatch: (item) => {}
		}
	);

	const entries = useMemo(() => {
		return (
			query.data?.flatMap((item) => item.entries) ||
			query.streaming.flatMap((item) => item.entries)
		);
	}, [query.streaming, query.data]);

	const items = useMemo(() => {
		if (!entries) return [];

		const ret: ExplorerItem[] = [];

		for (const item of entries) {
			if (settingsSnapshot.layoutMode !== 'media') ret.push(item);
			else {
				const { kind } = getExplorerItemData(item);

				if (kind === 'Video' || kind === 'Image') ret.push(item);
			}
		}

		// We sort on the frontend, as the backend streams in entries from cloud locations out of order
		const order = settingsSnapshot.order;
		if (order !== null) {
			const getValue = match(order.field)
				.with('name', () => (a: ItemData) => a.name)
				.with('sizeInBytes', () => (a: ItemData) => a.size.original)
				.with(
					'dateCreated',
					() => (a: ItemData) => (a.dateCreated !== null ? new Date(a.dateCreated) : null)
				)
				.with(
					'dateModified',
					() => (a: ItemData) =>
						a.dateModified !== null ? new Date(a.dateModified) : null
				)
				.exhaustive();

			return ret.sort((a, b) => {
				const aData = getExplorerItemData(a);
				const bData = getExplorerItemData(b);

				let result = 0;

				// Put hidden files first (if the files have a hidden property)
				if (
					'hidden' in a.item &&
					'hidden' in b.item &&
					a.item.hidden !== null &&
					b.item.hidden !== null
				)
					result = +b.item.hidden - +a.item.hidden;

				// Group files before folders (within the hidden groups)
				result = result || +(aData.kind === 'Folder') - +(bData.kind === 'Folder');

				// Finally sort by the user defined property & flip the result for descending order if needed
				const valueA = getValue(aData);
				const valueB = getValue(bData);
				result = result || compare(valueA, valueB) * (order.value === 'Asc' ? 1 : -1);

				return result;
			});
		}

		return ret;
	}, [entries, settingsSnapshot.layoutMode, settingsSnapshot.order]);

	const explorer = useExplorer({
		items,
		parent: path != null ? { type: 'Ephemeral', path } : undefined,
		settings: explorerSettings,
		layouts: { media: false }
	});

	useKeyDeleteFile(explorer.selectedItems, null);

	const { t } = useLocale();

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal
				left={
					<Tooltip label={t('add_location_tooltip')} className="w-max min-w-0 shrink">
						<AddLocationButton path={path} />
					</Tooltip>
				}
				right={<DefaultTopBarOptions />}
			/>
			<Explorer
				emptyNotice={
					<EmptyNotice
						loading={query.isFetching}
						icon={<Icon name="FolderNoSpace" size={128} />}
						message={t('location_empty_notice_message')}
					/>
				}
			/>
		</ExplorerContextProvider>
	);
});

export const Component = () => {
	const [pathParams] = useZodSearchParams(PathParamsSchema);

	const path = useDeferredValue(pathParams);

	useRouteTitle(path.path ?? '');

	return (
		<Suspense>
			<EphemeralNotice path={path.path ?? ''} />
			<EphemeralExplorer args={path} />
		</Suspense>
	);
};

// Compare two values and return a number based on their relative order
function compare<T extends string | number | Date | BigInt | null>(a: T, b: T) {
	if (a !== null && b !== null) {
		if (typeof a === 'string') {
			return a.localeCompare(b as string);
		} else {
			// We must avoid equality as Date doesn't support them but if a > b & b > a then a === b
			return a < b ? -1 : a > b ? 1 : 0;
		}
	}

	if (a === null && b !== null) return -1;
	if (a !== null && b === null) return 1;

	return 0;
}
