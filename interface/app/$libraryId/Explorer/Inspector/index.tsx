import { Image, Image_Light } from '@sd/assets/icons';
import clsx from 'clsx';
import dayjs from 'dayjs';
import {
	Barcode,
	CircleWavyCheck,
	Clock,
	Cube,
	Eraser,
	FolderOpen,
	Hash,
	Icon,
	Link,
	Lock,
	Path,
	Snowflake
} from 'phosphor-react';
import {
	type HTMLAttributes,
	type ReactNode,
	useCallback,
	useEffect,
	useMemo,
	useState
} from 'react';
import {
	type ExplorerItem,
	byteSize,
	getExplorerItemData,
	getItemFilePath,
	getItemObject,
	useItemsAsObjects,
	useLibraryQuery
} from '@sd/client';
import { Button, Divider, DropdownMenu, Tooltip, tw } from '@sd/ui';
import AssignTagMenuItems from '~/components/AssignTagMenuItems';
import { useIsDark } from '~/hooks';
import { isNonEmpty } from '~/util';
import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';
import { useExplorerStore } from '../store';
import { uniqueId, useExplorerItemData } from '../util';
import FavoriteButton from './FavoriteButton';
import Note from './Note';

export const InfoPill = tw.span`inline border border-transparent px-1 text-[11px] font-medium shadow shadow-app-shade/5 bg-app-selected rounded-md text-ink-dull`;
export const PlaceholderPill = tw.span`inline border px-1 text-[11px] shadow shadow-app-shade/10 rounded-md bg-transparent border-dashed border-app-active transition hover:text-ink-faint hover:border-ink-faint font-medium text-ink-faint/70`;

export const MetaContainer = tw.div`flex flex-col px-4 py-2 gap-1`;
export const MetaTitle = tw.h5`text-xs font-bold`;

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

export const Inspector = ({ showThumbnail = true, ...props }: Props) => {
	const explorer = useExplorerContext();

	const isDark = useIsDark();

	const selectedItems = useMemo(() => [...explorer.selectedItems], [explorer.selectedItems]);

	return (
		<div {...props}>
			{showThumbnail && (
				<div className="relative mb-2 flex aspect-square items-center justify-center px-2">
					{isNonEmpty(selectedItems) ? (
						<Thumbnails items={selectedItems} />
					) : (
						<img src={isDark ? Image : Image_Light} />
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
		</div>
	);
};

const Thumbnails = ({ items }: { items: ExplorerItem[] }) => {
	const explorerStore = useExplorerStore();

	const lastThreeItems = items.slice(-3).reverse();

	return (
		<>
			{lastThreeItems.map((item, i, thumbs) => (
				<FileThumb
					key={uniqueId(item)}
					data={item}
					loadOriginal
					frame
					blackBars={thumbs.length === 1}
					blackBarsSize={16}
					extension={thumbs.length > 1}
					pauseVideo={!!explorerStore.quickViewObject || thumbs.length > 1}
					className={clsx(
						thumbs.length > 1 && '!absolute',
						i === 0 && thumbs.length > 1 && 'z-30 !h-[76%] !w-[76%]',
						i === 1 && 'z-20 !h-[80%] !w-[80%] rotate-[-5deg]',
						i === 2 && 'z-10 !h-[84%] !w-[84%] rotate-[7deg]'
					)}
					childClassName={(type) =>
						type !== 'ICON' && thumbs.length > 1
							? 'shadow-md shadow-app-shade'
							: undefined
					}
				/>
			))}
		</>
	);
};

const SingleItemMetadata = ({ item }: { item: ExplorerItem }) => {
	const objectData = getItemObject(item);
	const readyToFetch = useIsFetchReady(item);
	const isNonIndexed = item.type === 'NonIndexedPath';

	const tags = useLibraryQuery(['tags.getForObject', objectData?.id ?? -1], {
		enabled: !!objectData && readyToFetch
	});

	const object = useLibraryQuery(['files.get', { id: objectData?.id ?? -1 }], {
		enabled: !!objectData && readyToFetch
	});

	let { data: fileFullPath } = useLibraryQuery(['files.getPath', objectData?.id ?? -1], {
		enabled: !!objectData && readyToFetch
	});

	if (fileFullPath == null) {
		switch (item.type) {
			case 'Location':
			case 'NonIndexedPath':
				fileFullPath = item.item.path;
		}
	}

	const { name, isDir, kind, size, casId, dateCreated, dateAccessed, dateModified, dateIndexed } =
		useExplorerItemData(item);

	const pubId = object?.data ? uniqueId(object?.data) : null;

	let extension, integrityChecksum;
	const filePathItem = getItemFilePath(item);
	if (filePathItem) {
		extension = 'extension' in filePathItem ? filePathItem.extension : null;
		integrityChecksum =
			'integrity_checksum' in filePathItem ? filePathItem.integrity_checksum : null;
	}

	return (
		<>
			<h3 className="truncate px-3 pb-1 pt-2 text-base font-bold">
				{name}
				{extension && `.${extension}`}
			</h3>

			{objectData && (
				<div className="mx-3 mb-0.5 mt-1 flex flex-row space-x-0.5">
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

				{isNonIndexed || (
					<MetaData icon={Barcode} label="Indexed" value={formatDate(dateIndexed)} />
				)}

				{isNonIndexed || (
					<MetaData icon={FolderOpen} label="Accessed" value={formatDate(dateAccessed)} />
				)}

				<MetaData
					icon={Path}
					label="Path"
					value={fileFullPath}
					onClick={() => {
						// TODO: Add toast notification
						fileFullPath && navigator.clipboard.writeText(fileFullPath);
					}}
				/>
			</MetaContainer>

			<Divider />

			<MetaContainer className="flex !flex-row flex-wrap gap-1 overflow-hidden">
				<InfoPill>{isDir ? 'Folder' : kind}</InfoPill>

				{extension && <InfoPill>{extension}</InfoPill>}

				{tags.data?.map((tag) => (
					<Tooltip key={tag.id} label={tag.name || ''} className="flex overflow-hidden">
						<InfoPill
							className="truncate !text-white"
							style={{ backgroundColor: tag.color + 'CC' }}
						>
							{tag.name}
						</InfoPill>
					</Tooltip>
				))}

				{objectData && (
					<DropdownMenu.Root
						trigger={<PlaceholderPill>Add Tag</PlaceholderPill>}
						side="left"
						sideOffset={5}
						alignOffset={-10}
					>
						<AssignTagMenuItems objects={[objectData]} />
					</DropdownMenu.Root>
				)}
			</MetaContainer>

			{!isDir && objectData && (
				<>
					<Note data={objectData} />

					<Divider />

					<MetaContainer>
						{isNonIndexed || (
							<MetaData icon={Snowflake} label="Content ID" value={casId} />
						)}

						{integrityChecksum && (
							<MetaData
								icon={CircleWavyCheck}
								label="Checksum"
								value={integrityChecksum}
							/>
						)}

						{isNonIndexed || <MetaData icon={Hash} label="Object ID" value={pubId} />}
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

	const tags = useLibraryQuery(['tags.list'], {
		enabled: readyToFetch && !explorerStore.isDragging,
		suspense: true
	});

	const tagsWithObjects = useLibraryQuery(
		['tags.getWithObjects', selectedObjects.map(({ id }) => id)],
		{ enabled: readyToFetch && !explorerStore.isDragging }
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

				{tags.data?.map((tag) => {
					const objectsWithTag = tagsWithObjects.data?.[tag.id] || [];

					if (objectsWithTag.length === 0) return null;

					return (
						<Tooltip key={tag.id} label={tag.name} className="flex overflow-hidden">
							<InfoPill
								className="truncate !text-white"
								style={{
									backgroundColor: tag.color + 'CC',
									opacity:
										objectsWithTag.length === selectedObjects.length ? 1 : 0.5
								}}
							>
								{tag.name} ({objectsWithTag.length})
							</InfoPill>
						</Tooltip>
					);
				})}

				{isNonEmpty(selectedObjects) && (
					<DropdownMenu.Root
						trigger={<PlaceholderPill>Add Tag</PlaceholderPill>}
						side="left"
						sideOffset={5}
						alignOffset={-10}
					>
						<AssignTagMenuItems objects={selectedObjects} />
					</DropdownMenu.Root>
				)}
			</MetaContainer>
		</>
	);
};

interface MetaDataProps {
	icon: Icon;
	label: string;
	value: ReactNode;
	onClick?: () => void;
}

const MetaData = ({ icon: Icon, label, value, onClick }: MetaDataProps) => {
	return (
		<div className="flex items-center text-xs text-ink-dull" onClick={onClick}>
			<Icon weight="bold" className="mr-2 shrink-0" />
			<span className="mr-2 flex-1 whitespace-nowrap">{label}</span>
			<Tooltip label={value} asChild>
				<span className="truncate break-all text-ink">{value || '--'}</span>
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
