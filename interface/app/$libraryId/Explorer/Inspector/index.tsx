import {
	Barcode,
	CircleWavyCheck,
	Clock,
	Cube,
	Eraser,
	FolderOpen,
	Hash,
	Link,
	Lock,
	Path,
	Icon as PhosphorIcon,
	Snowflake
} from '@phosphor-icons/react';
import clsx from 'clsx';
import dayjs from 'dayjs';
import {
	forwardRef,
	useCallback,
	useEffect,
	useMemo,
	useState,
	type HTMLAttributes,
	type ReactNode
} from 'react';
import { useLocation } from 'react-router';
import { Link as NavLink } from 'react-router-dom';
import Sticky from 'react-sticky-el';
import {
	FilePath,
	FilePathForFrontend,
	getExplorerItemData,
	getItemFilePath,
	humanizeSize,
	NonIndexedPathItem,
	Object,
	ObjectWithFilePaths,
	useBridgeQuery,
	useItemsAsObjects,
	useLibraryQuery,
	useSelector,
	type ExplorerItem
} from '@sd/client';
import { Button, Divider, DropdownMenu, toast, Tooltip, tw } from '@sd/ui';
import { LibraryIdParamsSchema } from '~/app/route-schemas';
import { Folder, Icon } from '~/components';
import { useLocale, useZodRouteParams } from '~/hooks';
import { isNonEmpty } from '~/util';

import { useExplorerContext } from '../Context';
import AssignTagMenuItems from '../ContextMenu/AssignTagMenuItems';
import { FileThumb } from '../FilePath/Thumb';
import { useQuickPreviewStore } from '../QuickPreview/store';
import { explorerStore } from '../store';
import { useExplorerItemData } from '../useExplorerItemData';
import { translateKindName, uniqueId } from '../util';
import { RenamableItemText } from '../View/RenamableItemText';
import FavoriteButton from './FavoriteButton';
import MediaData from './MediaData';
import Note from './Note';

export const InfoPill = tw.span`inline border border-transparent px-1 text-[11px] font-medium shadow shadow-app-shade/5 bg-app-selected rounded-md text-ink-dull`;
export const PlaceholderPill = tw.span`cursor-default inline border px-1 text-[11px] shadow shadow-app-shade/10 rounded-md bg-transparent border-dashed border-app-active transition hover:text-ink-faint hover:border-ink-faint font-medium text-ink-faint/70`;

export const MetaContainer = tw.div`flex flex-col px-4 py-2 gap-1`;
export const MetaTitle = tw.h5`text-xs font-bold text-ink`;

export const INSPECTOR_WIDTH = 260;

type MetadataDate = Date | { from: Date; to: Date } | null;

const formatDate = (date: MetadataDate | string | undefined, dateFormat: string) => {
	if (!date) return;
	if (date instanceof Date || typeof date === 'string') return dayjs(date).format(dateFormat);

	const { from, to } = date;

	const sameMonth = from.getMonth() === to.getMonth();
	const sameYear = from.getFullYear() === to.getFullYear();

	const format = ['D', !sameMonth && 'MMM', !sameYear && 'YYYY'].filter(Boolean).join(' ');

	return `${dayjs(from).format(format)} - ${dayjs(to).format(dateFormat)}`;
};

interface Props extends HTMLAttributes<HTMLDivElement> {
	showThumbnail?: boolean;
}

export const Inspector = forwardRef<HTMLDivElement, Props>(
	({ showThumbnail = true, style, ...props }, ref) => {
		const explorer = useExplorerContext();

		const pathname = useLocation().pathname;

		const selectedItems = useMemo(() => [...explorer.selectedItems], [explorer.selectedItems]);

		useEffect(() => {
			explorerStore.showMoreInfo = false;
		}, [pathname]);

		const { t } = useLocale();
		return (
			<div ref={ref} style={{ width: INSPECTOR_WIDTH, ...style }} {...props}>
				<Sticky stickyClassName="!top-[40px]" topOffset={-40}>
					{showThumbnail && (
						<div className="relative mb-2 flex aspect-square items-center justify-center px-2">
							{isNonEmpty(selectedItems) ? (
								<Thumbnails items={selectedItems} />
							) : (
								<Icon name="Image" />
							)}
						</div>
					)}

					<div className="flex select-text flex-col overflow-hidden rounded-lg border border-app-line bg-app-box py-0.5 shadow-app-shade/10">
						{!isNonEmpty(selectedItems) ? (
							<div className="flex h-[390px] items-center justify-center text-sm text-ink-dull">
								{t('nothing_selected')}
							</div>
						) : selectedItems.length === 1 ? (
							<SingleItemMetadata item={selectedItems[0]} />
						) : (
							<MultiItemMetadata items={selectedItems} />
						)}
					</div>
				</Sticky>
			</div>
		);
	}
);

const Thumbnails = ({ items }: { items: ExplorerItem[] }) => {
	const quickPreviewStore = useQuickPreviewStore();

	const lastThreeItems = items.slice(-3).reverse();

	return (
		<>
			{lastThreeItems.map((item, i, thumbs) => (
				<FileThumb
					key={uniqueId(item)}
					data={item}
					loadOriginal={getItemFilePath(item)?.extension !== 'pdf' && thumbs.length === 1}
					frame
					blackBars={thumbs.length === 1}
					blackBarsSize={16}
					extension={thumbs.length > 1}
					pauseVideo={quickPreviewStore.open || thumbs.length > 1}
					className={clsx(
						thumbs.length > 1 && '!absolute',
						i === 0 && thumbs.length > 1 && 'z-30 !h-[76%] !w-[76%]',
						i === 1 && 'z-20 !h-4/5 !w-4/5 rotate-[-5deg]',
						i === 2 && 'z-10 !h-[84%] !w-[84%] rotate-[7deg]'
					)}
					childClassName={(type) =>
						type !== 'icon' && thumbs.length > 1
							? 'shadow-md shadow-app-shade'
							: undefined
					}
					isSidebarPreview={true}
				/>
			))}
		</>
	);
};

export const SingleItemMetadata = ({ item }: { item: ExplorerItem }) => {
	let objectData: Object | ObjectWithFilePaths | null = null;
	let filePathData: FilePath | FilePathForFrontend | null = null;
	let ephemeralPathData: NonIndexedPathItem | null = null;

	const { t, dateFormat } = useLocale();

	const result = useLibraryQuery(['locations.list']);
	const locations = result.data || [];

	switch (item.type) {
		case 'NonIndexedPath': {
			ephemeralPathData = item.item;
			break;
		}
		case 'Path': {
			objectData = item.item.object;
			filePathData = item.item;
			break;
		}
		case 'Object': {
			objectData = item.item;
			filePathData = item.item.file_paths[0] ?? null;
			break;
		}
		case 'SpacedropPeer': {
			// objectData = item.item as unknown as Object;
			// filePathData = item.item.file_paths[0] ?? null;
			break;
		}
	}

	const uniqueLocationIds = useMemo(() => {
		return item.type === 'Object'
			? [
					...new Set(
						(item.item?.file_paths || []).map((fp) => fp.location_id).filter(Boolean)
					)
				]
			: item.type === 'Path'
				? [item.item.location_id]
				: [];
	}, [item]);

	const fileLocations =
		locations?.filter((location) => uniqueLocationIds.includes(location.id)) || [];

	const readyToFetch = useIsFetchReady(item);

	const tagsQuery = useLibraryQuery(['tags.getForObject', objectData?.id ?? -1], {
		enabled: objectData != null && readyToFetch
	});
	const tags = tagsQuery.data;

	// const labels = useLibraryQuery(['labels.getForObject', objectData?.id ?? -1], {
	// 	enabled: objectData != null && readyToFetch
	// });

	const { libraryId } = useZodRouteParams(LibraryIdParamsSchema);

	const queriedFullPath = useLibraryQuery(['files.getPath', filePathData?.id ?? -1], {
		enabled: filePathData != null && readyToFetch
	});

	const duplicateFilePaths = useLibraryQuery(['files.getDuplicates', objectData?.id ?? -1], {
		enabled: objectData != null && readyToFetch
	});

	const filesMediaData = useLibraryQuery(['files.getMediaData', objectData?.id ?? -1], {
		enabled: objectData != null && readyToFetch
	});

	const ephemeralLocationMediaData = useBridgeQuery(
		['ephemeralFiles.getMediaData', ephemeralPathData != null ? ephemeralPathData.path : ''],
		{
			enabled: ephemeralPathData != null && readyToFetch
		}
	);

	const mediaData = filesMediaData.data ?? ephemeralLocationMediaData.data ?? null;

	const fullPath = queriedFullPath.data ?? ephemeralPathData?.path;

	const { isDir, kind, size, casId, dateCreated, dateAccessed, dateModified, dateIndexed } =
		useExplorerItemData(item);

	const pubId = objectData != null ? uniqueId(objectData) : null;

	let extension, integrityChecksum;

	if (filePathData != null) {
		extension = filePathData.extension;
		integrityChecksum =
			'integrity_checksum' in filePathData ? filePathData.integrity_checksum : null;
	}

	if (ephemeralPathData != null) {
		extension = ephemeralPathData.extension;
	}

	return (
		<>
			<div className="px-2 pb-1 pt-2">
				<RenamableItemText
					item={item}
					toggleBy="click"
					lines={2}
					editLines={2}
					selected
					allowHighlight={false}
					className="!text-base !font-bold !text-ink"
				/>
			</div>

			{objectData && (
				<div className="mx-3 mb-0.5 mt-1 flex flex-row space-x-0.5 text-ink">
					<Tooltip label={t('favorite')}>
						<FavoriteButton data={objectData} />
					</Tooltip>

					<Tooltip label={t('encrypt')}>
						<Button size="icon">
							<Lock className="size-[18px]" />
						</Button>
					</Tooltip>
					<Tooltip label={t('share')}>
						<Button size="icon">
							<Link className="size-[18px]" />
						</Button>
					</Tooltip>
				</div>
			)}

			<Divider />

			<MetaContainer>
				<MetaData
					icon={Cube}
					label={t('size')}
					value={
						!!ephemeralPathData && ephemeralPathData.is_dir
							? null
							: `${size.value} ${t(`size_${size.unit.toLowerCase()}`)}`
					}
				/>

				<MetaData
					icon={Clock}
					label={t('created')}
					value={formatDate(dateCreated, dateFormat)}
				/>

				<MetaData
					icon={Eraser}
					label={t('modified')}
					value={formatDate(dateModified, dateFormat)}
				/>

				{ephemeralPathData != null || (
					<MetaData
						icon={Barcode}
						label={t('indexed')}
						value={formatDate(dateIndexed, dateFormat)}
					/>
				)}

				{ephemeralPathData != null || (
					<MetaData
						icon={FolderOpen}
						label={t('accessed')}
						value={formatDate(dateAccessed, dateFormat)}
					/>
				)}

				<MetaData
					icon={Path}
					label={t('path')}
					value={fullPath}
					onClick={() => {
						if (fullPath) {
							navigator.clipboard.writeText(fullPath);
							toast.info(t('path_copied_to_clipboard_title'));
						}
					}}
				/>
			</MetaContainer>

			{fileLocations.length > 0 && (
				<MetaContainer>
					<MetaTitle>{t('locations')}</MetaTitle>
					<div className="flex flex-wrap gap-2">
						{fileLocations.map((location) => (
							<NavLink to={`/${libraryId}/location/${location.id}`} key={location.id}>
								<div className="flex flex-row rounded bg-app-hover/60 px-1 py-0.5 hover:bg-app-selected">
									<Folder size={18} />
									<span className="ml-1 text-xs text-ink">{location.name}</span>
								</div>
							</NavLink>
						))}
					</div>
				</MetaContainer>
			)}

			{mediaData && <MediaData data={mediaData} />}

			<MetaContainer className="flex !flex-row flex-wrap gap-1 overflow-hidden">
				<InfoPill>{isDir ? t('folder') : translateKindName(kind)}</InfoPill>

				{extension && <InfoPill>{extension}</InfoPill>}

				{/* {labels.data?.map((label) => (
					<InfoPill key={label.id} className="truncate !text-white">
						{label.name}
					</InfoPill>
				))} */}

				{tags?.map((tag) => (
					<NavLink key={tag.id} to={`/${libraryId}/tag/${tag.id}`}>
						<Tooltip label={tag.name || ''} className="flex overflow-hidden">
							<InfoPill
								className="cursor-pointer truncate !text-white"
								style={{ backgroundColor: tag.color + 'CC' }}
							>
								{tag.name}
							</InfoPill>
						</Tooltip>
					</NavLink>
				))}

				{item.type === 'Object' ||
					(item.type === 'Path' && (
						<DropdownMenu.Root
							trigger={<PlaceholderPill>{t('add_tag')}</PlaceholderPill>}
							side="left"
							className="z-[101]"
							sideOffset={5}
							alignOffset={-10}
						>
							<AssignTagMenuItems items={[item]} />
						</DropdownMenu.Root>
					))}
			</MetaContainer>

			{!isDir && objectData && (
				<>
					<Note data={objectData} />

					<Divider />

					<MetaContainer>
						<MetaData icon={Snowflake} label={t('content_id')} value={casId} />

						{integrityChecksum && (
							<MetaData
								icon={CircleWavyCheck}
								label={t('checksum')}
								value={integrityChecksum}
							/>
						)}

						<MetaData icon={Hash} label={t('object_id')} value={pubId} />
					</MetaContainer>
				</>
			)}
		</>
	);
};

const MultiItemMetadata = ({ items }: { items: ExplorerItem[] }) => {
	const isDragSelecting = useSelector(explorerStore, (s) => s.isDragSelecting);

	const selectedObjects = useItemsAsObjects(items);

	const readyToFetch = useIsFetchReady(items);

	const { libraryId } = useZodRouteParams(LibraryIdParamsSchema);

	const tagsQuery = useLibraryQuery(['tags.list'], {
		enabled: readyToFetch && !isDragSelecting,
		suspense: true
	});
	const tags = tagsQuery.data;

	// const labels = useLibraryQuery(['labels.list'], {
	// 	enabled: readyToFetch && !isDragSelecting,
	// 	suspense: true
	// });

	const tagsWithObjects = useLibraryQuery(
		['tags.getWithObjects', selectedObjects.map(({ id }) => id)],
		{ enabled: readyToFetch && !isDragSelecting }
	);

	// const labelsWithObjects = useLibraryQuery(
	// 	['labels.getWithObjects', selectedObjects.map(({ id }) => id)],
	// 	{ enabled: readyToFetch && !isDragSelecting }
	// );

	const getDate = useCallback((metadataDate: MetadataDate, date: Date) => {
		date.setHours(0, 0, 0, 0);

		if (!metadataDate) {
			metadataDate = date;
		} else if (metadataDate instanceof Date && date.getTime() !== metadataDate.getTime()) {
			metadataDate = { from: metadataDate, to: date };
		} else if ('from' in metadataDate && date < metadataDate.from) {
			metadataDate.from = date;
		} else if ('to' in metadataDate && date > metadataDate.to) {
			metadataDate.to = date;
		}

		return metadataDate;
	}, []);

	const metadata = useMemo(
		() =>
			items.reduce(
				(metadata, item) => {
					const { kind, size, dateCreated, dateAccessed, dateModified, dateIndexed } =
						getExplorerItemData(item);
					if (item.type !== 'NonIndexedPath' || !item.item.is_dir) {
						metadata.size = (metadata.size ?? 0n) + size.bytes;
					}

					if (dateCreated)
						metadata.created = getDate(metadata.created, new Date(dateCreated));

					if (dateModified)
						metadata.modified = getDate(metadata.modified, new Date(dateModified));

					if (dateIndexed)
						metadata.indexed = getDate(metadata.indexed, new Date(dateIndexed));

					if (dateAccessed)
						metadata.accessed = getDate(metadata.accessed, new Date(dateAccessed));

					metadata.types.add(item.type);

					const kindItems = metadata.kinds.get(kind);
					if (!kindItems) metadata.kinds.set(kind, [item]);
					else metadata.kinds.set(kind, [...kindItems, item]);

					return metadata;
				},
				{ size: null, indexed: null, types: new Set(), kinds: new Map() } as {
					size: bigint | null;
					created: MetadataDate;
					modified: MetadataDate;
					indexed: MetadataDate;
					accessed: MetadataDate;
					types: Set<ExplorerItem['type']>;
					kinds: Map<string, ExplorerItem[]>;
				}
			),
		[items, getDate]
	);

	const { t, dateFormat } = useLocale();

	const onlyNonIndexed = metadata.types.has('NonIndexedPath') && metadata.types.size === 1;
	const filesSize = humanizeSize(metadata.size);

	return (
		<>
			<MetaContainer>
				<MetaData
					icon={Cube}
					label={t('size')}
					value={
						metadata.size !== null
							? `${filesSize.value} ${t(`size_${filesSize.unit.toLowerCase()}s`)}`
							: null
					}
				/>
				<MetaData
					icon={Clock}
					label={t('created')}
					value={formatDate(metadata.created, dateFormat)}
				/>
				<MetaData
					icon={Eraser}
					label={t('modified')}
					value={formatDate(metadata.modified, dateFormat)}
				/>
				{onlyNonIndexed || (
					<MetaData
						icon={Barcode}
						label={t('indexed')}
						value={formatDate(metadata.indexed, dateFormat)}
					/>
				)}
				{onlyNonIndexed || (
					<MetaData
						icon={FolderOpen}
						label={t('accessed')}
						value={formatDate(metadata.accessed, dateFormat)}
					/>
				)}
			</MetaContainer>

			<Divider />

			<MetaContainer className="flex !flex-row flex-wrap gap-1 overflow-hidden">
				{[...metadata.kinds].map(([kind, items]) => (
					<InfoPill key={kind}>{`${translateKindName(kind)} (${items.length})`}</InfoPill>
				))}

				{/* {labels.data?.map((label) => {
					const objectsWithLabel = labelsWithObjects.data?.[label.id] ?? [];

					if (objectsWithLabel.length === 0) return null;

					return (
						<InfoPill
							key={label.id}
							className="!text-white"
							style={{
								opacity:
									objectsWithLabel.length === selectedObjects.length ? 1 : 0.5
							}}
						>
							{label.name} ({objectsWithLabel.length})
						</InfoPill>
					);
				})} */}

				{tags?.map((tag) => {
					const objectsWithTag = tagsWithObjects.data?.[tag.id] ?? [];

					if (objectsWithTag.length === 0) return null;

					return (
						<NavLink key={tag.id} to={`/${libraryId}/tag/${tag.id}`}>
							<Tooltip label={tag.name} className="flex overflow-hidden">
								<InfoPill
									className="cursor-pointer truncate !text-white"
									style={{
										backgroundColor: tag.color + 'CC',
										opacity:
											objectsWithTag.length === selectedObjects.length
												? 1
												: 0.5
									}}
								>
									{tag.name} ({objectsWithTag.length})
								</InfoPill>
							</Tooltip>
						</NavLink>
					);
				})}

				{isNonEmpty(selectedObjects) && (
					<DropdownMenu.Root
						trigger={<PlaceholderPill>{t('add_tag')}</PlaceholderPill>}
						side="left"
						sideOffset={5}
						alignOffset={-10}
					>
						<AssignTagMenuItems
							items={items.flatMap((item) => {
								if (item.type === 'Object' || item.type === 'Path') return [item];
								else return [];
							})}
						/>
					</DropdownMenu.Root>
				)}
			</MetaContainer>
		</>
	);
};

interface MetaDataProps {
	icon?: PhosphorIcon;
	label: string;
	value: ReactNode;
	tooltipValue?: ReactNode;
	onClick?: () => void;
}

export const MetaData = ({ icon: Icon, label, value, tooltipValue, onClick }: MetaDataProps) => {
	return (
		<div className="flex content-start justify-start text-xs text-ink-dull" onClick={onClick}>
			{Icon && <Icon weight="bold" className="mr-2 shrink-0" />}
			<span className="mr-2 flex flex-1 items-start justify-items-start whitespace-nowrap">
				{label}
			</span>
			<Tooltip
				label={tooltipValue || value}
				className="truncate whitespace-pre text-ink"
				tooltipClassName="max-w-none"
			>
				{value ?? '--'}
			</Tooltip>
		</div>
	);
};

const useIsFetchReady = (item: ExplorerItem | ExplorerItem[]) => {
	const [readyToFetch, setReadyToFetch] = useState(false);

	useEffect(() => {
		setReadyToFetch(false);

		const timeout = setTimeout(() => setReadyToFetch(true), 350);
		return () => clearTimeout(timeout);
	}, [item]);

	return readyToFetch;
};
