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
	byteSize,
	FilePath,
	FilePathWithObject,
	getExplorerItemData,
	getItemFilePath,
	NonIndexedPathItem,
	Object,
	ObjectKindEnum,
	ObjectWithFilePaths,
	useBridgeQuery,
	useCache,
	useItemsAsObjects,
	useLibraryQuery,
	useNodes,
	type ExplorerItem
} from '@sd/client';
import { Button, Divider, DropdownMenu, toast, Tooltip, tw } from '@sd/ui';
import { LibraryIdParamsSchema } from '~/app/route-schemas';
import { Folder, Icon } from '~/components';
import { useZodRouteParams } from '~/hooks';
import { isNonEmpty } from '~/util';

import { useExplorerContext } from '../Context';
import AssignTagMenuItems from '../ContextMenu/AssignTagMenuItems';
import { FileThumb } from '../FilePath/Thumb';
import { useQuickPreviewStore } from '../QuickPreview/store';
import { getExplorerStore, useExplorerStore } from '../store';
import { uniqueId, useExplorerItemData } from '../util';
import FavoriteButton from './FavoriteButton';
import MediaData from './MediaData';
import Note from './Note';

export const InfoPill = tw.span`inline border border-transparent px-1 text-[11px] font-medium shadow shadow-app-shade/5 bg-app-selected rounded-md text-ink-dull`;
export const PlaceholderPill = tw.span`cursor-default inline border px-1 text-[11px] shadow shadow-app-shade/10 rounded-md bg-transparent border-dashed border-app-active transition hover:text-ink-faint hover:border-ink-faint font-medium text-ink-faint/70`;

export const MetaContainer = tw.div`flex flex-col px-4 py-2 gap-1`;
export const MetaTitle = tw.h5`text-xs font-bold text-ink`;

export const INSPECTOR_WIDTH = 260;

type MetadataDate = Date | { from: Date; to: Date } | null;

const DATE_FORMAT = 'D MMM YYYY';
const formatDate = (date: MetadataDate | string | undefined) => {
	if (!date) return;
	if (date instanceof Date || typeof date === 'string') return dayjs(date).format(DATE_FORMAT);

	const { from, to } = date;

	const sameMonth = from.getMonth() === to.getMonth();
	const sameYear = from.getFullYear() === to.getFullYear();

	const format = ['D', !sameMonth && 'MMM', !sameYear && 'YYYY'].filter(Boolean).join(' ');

	return `${dayjs(from).format(format)} - ${dayjs(to).format(DATE_FORMAT)}`;
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
			getExplorerStore().showMoreInfo = false;
		}, [pathname]);

		return (
			<div ref={ref} style={{ width: INSPECTOR_WIDTH, ...style }} {...props}>
				<Sticky
					scrollElement={explorer.scrollRef.current || undefined}
					stickyClassName="!top-[40px]"
					topOffset={-40}
				>
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
								Nothing selected
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
						i === 1 && 'z-20 !h-[80%] !w-[80%] rotate-[-5deg]',
						i === 2 && 'z-10 !h-[84%] !w-[84%] rotate-[7deg]'
					)}
					childClassName={(type) =>
						type.variant !== 'icon' && thumbs.length > 1
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
	let filePathData: FilePath | FilePathWithObject | null = null;
	let ephemeralPathData: NonIndexedPathItem | null = null;

	const result = useLibraryQuery(['locations.list']);
	useNodes(result.data?.nodes);
	const locations = useCache(result.data?.items);

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
	useNodes(tagsQuery.data?.nodes);
	const tags = useCache(tagsQuery.data?.items);

	const { libraryId } = useZodRouteParams(LibraryIdParamsSchema);

	const queriedFullPath = useLibraryQuery(['files.getPath', filePathData?.id ?? -1], {
		enabled: filePathData != null && readyToFetch
	});

	const filesMediaData = useLibraryQuery(['files.getMediaData', objectData?.id ?? -1], {
		enabled: objectData?.kind === ObjectKindEnum.Image && readyToFetch
	});

	const ephemeralLocationMediaData = useBridgeQuery(
		['ephemeralFiles.getMediaData', ephemeralPathData != null ? ephemeralPathData.path : ''],
		{
			enabled: ephemeralPathData?.kind === ObjectKindEnum.Image && readyToFetch
		}
	);

	const mediaData = filesMediaData ?? ephemeralLocationMediaData ?? null;

	const fullPath = queriedFullPath.data ?? ephemeralPathData?.path;

	const { name, isDir, kind, size, casId, dateCreated, dateAccessed, dateModified, dateIndexed } =
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
			<h3 className="truncate px-3 pb-1 pt-2 text-base font-bold text-ink">
				{name}
				{extension && `.${extension}`}
			</h3>

			{objectData && (
				<div className="mx-3 mb-0.5 mt-1 flex flex-row space-x-0.5 text-ink">
					<Tooltip label="Favorite">
						<FavoriteButton data={objectData} />
					</Tooltip>

					<Tooltip label="Encrypt">
						<Button size="icon">
							<Lock className="h-[18px] w-[18px]" />
						</Button>
					</Tooltip>
					<Tooltip label="Share">
						<Button size="icon">
							<Link className="h-[18px] w-[18px]" />
						</Button>
					</Tooltip>
				</div>
			)}

			<Divider />

			<MetaContainer>
				<MetaData icon={Cube} label="Size" value={`${size}`} />

				<MetaData icon={Clock} label="Created" value={formatDate(dateCreated)} />

				<MetaData icon={Eraser} label="Modified" value={formatDate(dateModified)} />

				{ephemeralPathData != null || (
					<MetaData icon={Barcode} label="Indexed" value={formatDate(dateIndexed)} />
				)}

				{ephemeralPathData != null || (
					<MetaData icon={FolderOpen} label="Accessed" value={formatDate(dateAccessed)} />
				)}

				<MetaData
					icon={Path}
					label="Path"
					value={fullPath}
					onClick={() => {
						if (fullPath) {
							navigator.clipboard.writeText(fullPath);
							toast.info('Copied path to clipboard');
						}
					}}
				/>
			</MetaContainer>

			{fileLocations.length > 0 && (
				<MetaContainer>
					<MetaTitle>Locations</MetaTitle>
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

			{mediaData.data && <MediaData data={mediaData.data} />}

			<MetaContainer className="flex !flex-row flex-wrap gap-1 overflow-hidden">
				<InfoPill>{isDir ? 'Folder' : kind}</InfoPill>

				{extension && <InfoPill>{extension}</InfoPill>}

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
							trigger={<PlaceholderPill>Add Tag</PlaceholderPill>}
							side="left"
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
						<MetaData icon={Snowflake} label="Content ID" value={casId} />

						{integrityChecksum && (
							<MetaData
								icon={CircleWavyCheck}
								label="Checksum"
								value={integrityChecksum}
							/>
						)}

						<MetaData icon={Hash} label="Object ID" value={pubId} />
					</MetaContainer>
				</>
			)}
		</>
	);
};

const MultiItemMetadata = ({ items }: { items: ExplorerItem[] }) => {
	const explorerStore = useExplorerStore();

	const selectedObjects = useItemsAsObjects(items);

	const readyToFetch = useIsFetchReady(items);

	const { libraryId } = useZodRouteParams(LibraryIdParamsSchema);

	const tagsQuery = useLibraryQuery(['tags.list'], {
		enabled: readyToFetch && !explorerStore.isDragSelecting,
		suspense: true
	});
	useNodes(tagsQuery.data?.nodes);
	const tags = useCache(tagsQuery.data?.items);

	const tagsWithObjects = useLibraryQuery(
		['tags.getWithObjects', selectedObjects.map(({ id }) => id)],
		{ enabled: readyToFetch && !explorerStore.isDragSelecting }
	);

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

					metadata.size += size.original;

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
				{ size: BigInt(0), indexed: null, types: new Set(), kinds: new Map() } as {
					size: bigint;
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

	const onlyNonIndexed = metadata.types.has('NonIndexedPath') && metadata.types.size === 1;

	return (
		<>
			<MetaContainer>
				<MetaData icon={Cube} label="Size" value={`${byteSize(metadata.size)}`} />
				<MetaData icon={Clock} label="Created" value={formatDate(metadata.created)} />
				<MetaData icon={Eraser} label="Modified" value={formatDate(metadata.modified)} />
				{onlyNonIndexed || (
					<MetaData icon={Barcode} label="Indexed" value={formatDate(metadata.indexed)} />
				)}
				{onlyNonIndexed || (
					<MetaData
						icon={FolderOpen}
						label="Accessed"
						value={formatDate(metadata.accessed)}
					/>
				)}
			</MetaContainer>

			<Divider />

			<MetaContainer className="flex !flex-row flex-wrap gap-1 overflow-hidden">
				{[...metadata.kinds].map(([kind, items]) => (
					<InfoPill key={kind}>{`${kind} (${items.length})`}</InfoPill>
				))}

				{tags?.map((tag) => {
					const objectsWithTag = tagsWithObjects.data?.[tag.id] || [];

					if (objectsWithTag.length === 0) return null;

					return (
						<NavLink key={tag.id} to={`/${libraryId}/tag/${tag.id}`}>
							<Tooltip key={tag.id} label={tag.name} className="flex overflow-hidden">
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
						trigger={<PlaceholderPill>Add Tag</PlaceholderPill>}
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
		<div className="flex items-center text-xs text-ink-dull" onClick={onClick}>
			{Icon && <Icon weight="bold" className="mr-2 shrink-0" />}
			<span className="mr-2 flex-1 whitespace-nowrap">{label}</span>
			<Tooltip label={tooltipValue || value} asChild>
				<span className="truncate break-all text-ink">{value ?? '--'}</span>
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
