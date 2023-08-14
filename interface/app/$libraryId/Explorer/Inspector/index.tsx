// import types from '../../constants/file-types.json';
import { Image, Image_Light } from '@sd/assets/icons';
import clsx from 'clsx';
import dayjs from 'dayjs';
import {
	Barcode,
	CircleWavyCheck,
	Clock,
	Cube,
	Hash,
	Link,
	Lock,
	Path,
	Snowflake
} from 'phosphor-react';
import { type HTMLAttributes, type ReactNode, useEffect, useState } from 'react';
import {
	type ExplorerItem,
	type Location,
	type Tag,
	getExplorerItemData,
	getItemFilePath,
	getItemObject,
	isPath,
	useLibraryQuery
} from '@sd/client';
import { Button, Divider, DropdownMenu, Tooltip, tw } from '@sd/ui';
import { useIsDark } from '~/hooks';
import AssignTagMenuItems from '../ContextMenu/Object/AssignTagMenuItems';
import FileThumb from '../FilePath/Thumb';
import { useExplorerStore } from '../store.js';
import { uniqueId } from '../util';
import FavoriteButton from './FavoriteButton';
import Note from './Note';

export const InfoPill = tw.span`inline border border-transparent px-1 text-[11px] font-medium shadow shadow-app-shade/5 bg-app-selected rounded-md text-ink-dull`;
export const PlaceholderPill = tw.span`inline border px-1 text-[11px] shadow shadow-app-shade/10 rounded-md bg-transparent border-dashed border-app-active transition hover:text-ink-faint hover:border-ink-faint font-medium text-ink-faint/70`;

export const MetaContainer = tw.div`flex flex-col px-4 py-1.5`;
export const MetaTitle = tw.h5`text-xs font-bold`;
export const MetaKeyName = tw.h5`text-xs flex-shrink-0 flex-wrap-0`;
export const MetaValue = tw.p`text-xs break-all text-ink truncate`;

const MetaTextLine = tw.div`flex items-center my-0.5 text-xs text-ink-dull`;

const InspectorIcon = ({ component: Icon, ...props }: any) => (
	<Icon weight="bold" {...props} className={clsx('mr-2 shrink-0', props.className)} />
);

const ContainerWithDivider = ({
	children,
	before = false,
	...props
}: Parameters<typeof MetaContainer>[0] & { before?: boolean }) => {
	if (Array.isArray(children)) {
		const childrens = children.filter(Boolean) as ReactNode[];
		children = childrens.length === 0 ? null : childrens;
	}

	return children ? (
		<>
			{before && <Divider />}

			<MetaContainer {...props}>{children}</MetaContainer>

			{before || <Divider />}
		</>
	) : null;
};

interface Props extends HTMLAttributes<HTMLDivElement> {
	context?: Location | Tag;
	data?: ExplorerItem;
	showThumbnail?: boolean;
}

export const Inspector = ({ data, context, showThumbnail = true, ...props }: Props) => {
	const dataId = data ? uniqueId(data) : null;
	const isDark = useIsDark();
	const objectData = data ? getItemObject(data) : null;
	const explorerStore = useExplorerStore();
	const [readyToFetch, setReadyToFetch] = useState(false);

	// Prevents the inspector from fetching data when the user is navigating quickly
	useEffect(() => {
		const timeout = setTimeout(() => {
			setReadyToFetch(true);
		}, 350);
		return () => clearTimeout(timeout);
	}, [dataId]);

	// this is causing LAG
	const tags = useLibraryQuery(['tags.getForObject', objectData?.id ?? -1], {
		enabled: !!objectData && readyToFetch
	});

	const fullObjectData = useLibraryQuery(['files.get', { id: objectData?.id ?? -1 }], {
		enabled: !!objectData && readyToFetch
	});

	let { data: fileFullPath } = useLibraryQuery(['files.getPath', objectData?.id ?? -1], {
		enabled: !!objectData && readyToFetch
	});

	if (fileFullPath == null && data) {
		switch (data.type) {
			case 'Location':
			case 'NonIndexedPath':
				fileFullPath = data.item.path;
		}
	}

	if (!data)
		return (
			<div {...props}>
				<div className="flex w-full flex-col items-center justify-center">
					<img src={isDark ? Image : Image_Light} />
					<div
						className="mt-[15px] flex h-[390px] w-[245px] select-text items-center justify-center
	rounded-lg border border-app-line bg-app-box py-0.5 shadow-app-shade/10"
					>
						<p className="text-sm text-ink-dull">Nothing selected</p>
					</div>
				</div>
			</div>
		);

	const { name, isDir, kind, size, casId, dateCreated, dateIndexed } = getExplorerItemData(data);

	const pubId = fullObjectData?.data ? uniqueId(fullObjectData?.data) : null;
	const mediaData = fullObjectData?.data?.media_data;

	let extension, integrityChecksum;
	const filePathItem = getItemFilePath(data);
	if (filePathItem) {
		extension = 'extension' in filePathItem ? filePathItem.extension : null;
		integrityChecksum =
			'integrity_checksum' in filePathItem ? filePathItem.integrity_checksum : null;
	}

	return (
		<div {...props}>
			{showThumbnail && (
				<div className="mb-2 aspect-square">
					<FileThumb
						pauseVideo={!!explorerStore.quickViewObject}
						loadOriginal
						size={null}
						data={data}
						className="mx-auto"
					/>
				</div>
			)}

			<div className="flex w-full select-text flex-col overflow-hidden rounded-lg border border-app-line bg-app-box py-0.5 shadow-app-shade/10">
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

				{isPath(data) && context && 'path' in context && (
					<ContainerWithDivider>
						<MetaTitle>URI</MetaTitle>
						<MetaValue>
							{`${context.path}/${data.item.materialized_path}${data.item.name}${
								data.item.is_dir ? `.${data.item.extension}` : '/'
							}`}
						</MetaValue>
					</ContainerWithDivider>
				)}

				<ContainerWithDivider>
					<div className="flex flex-wrap gap-1 overflow-hidden">
						<InfoPill>{isDir ? 'Folder' : kind}</InfoPill>

						{extension && <InfoPill>{extension}</InfoPill>}

						{tags.data?.map((tag) => (
							<Tooltip
								key={tag.id}
								label={tag.name || ''}
								className="flex overflow-hidden"
							>
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
								<AssignTagMenuItems objectId={objectData.id} />
							</DropdownMenu.Root>
						)}
					</div>
				</ContainerWithDivider>

				<ContainerWithDivider className="!flex-row space-x-2">
					{!isDir && size && (
						<MetaTextLine>
							<InspectorIcon component={Cube} />
							<span className="mr-1.5">Size</span>
							<MetaValue>{`${size}`}</MetaValue>
						</MetaTextLine>
					)}

					{mediaData && (
						<MetaTextLine>
							<InspectorIcon component={Clock} />
							<span className="mr-1.5">Duration</span>
							<MetaValue>{mediaData.duration_seconds}</MetaValue>
						</MetaTextLine>
					)}
				</ContainerWithDivider>

				<MetaContainer>
					<Tooltip label={dayjs(dateCreated).format('h:mm:ss a')}>
						<MetaTextLine>
							<InspectorIcon component={Clock} />
							<MetaKeyName className="mr-1.5">Created</MetaKeyName>
							<MetaValue>{dayjs(dateCreated).format('MMM Do YYYY')}</MetaValue>
						</MetaTextLine>
					</Tooltip>

					{dateIndexed && (
						<Tooltip label={dayjs(dateIndexed).format('h:mm:ss a')}>
							<MetaTextLine>
								<InspectorIcon component={Barcode} />
								<MetaKeyName className="mr-1.5">Indexed</MetaKeyName>
								<MetaValue>{dayjs(dateIndexed).format('MMM Do YYYY')}</MetaValue>
							</MetaTextLine>
						</Tooltip>
					)}

					{fileFullPath && (
						<Tooltip label={fileFullPath}>
							<MetaTextLine>
								<InspectorIcon component={Path} />
								<MetaKeyName className="mr-1.5">Path</MetaKeyName>
								<MetaValue>{fileFullPath}</MetaValue>
							</MetaTextLine>
						</Tooltip>
					)}
				</MetaContainer>

				{!isDir && (
					<>
						{objectData && <Note data={objectData} />}

						<ContainerWithDivider before={true}>
							{casId && (
								<Tooltip label={casId}>
									<MetaTextLine>
										<InspectorIcon component={Snowflake} />
										<MetaKeyName className="mr-1.5">Content ID</MetaKeyName>
										<MetaValue>{casId}</MetaValue>
									</MetaTextLine>
								</Tooltip>
							)}

							{integrityChecksum && (
								<Tooltip label={integrityChecksum}>
									<MetaTextLine>
										<InspectorIcon component={CircleWavyCheck} />
										<MetaKeyName className="mr-1.5">Checksum</MetaKeyName>
										<MetaValue>{integrityChecksum}</MetaValue>
									</MetaTextLine>
								</Tooltip>
							)}

							{pubId && (
								<Tooltip label={pubId || ''}>
									<MetaTextLine>
										<InspectorIcon component={Hash} />
										<MetaKeyName className="mr-1.5">Object ID</MetaKeyName>
										<MetaValue>{pubId}</MetaValue>
									</MetaTextLine>
								</Tooltip>
							)}
						</ContainerWithDivider>
					</>
				)}
			</div>
		</div>
	);
};
