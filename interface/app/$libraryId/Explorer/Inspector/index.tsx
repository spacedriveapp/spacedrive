// import types from '../../constants/file-types.json';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { Barcode, CircleWavyCheck, Clock, Cube, Hash, Link, Lock, Snowflake } from 'phosphor-react';
import { ComponentProps, useEffect, useState } from 'react';
import {
	ExplorerContext,
	ExplorerItem,
	ObjectKind,
	formatBytes,
	useLibraryQuery
} from '@sd/client';
import { Button, Divider, DropdownMenu, Tooltip, tw } from '@sd/ui';
import { useExplorerStore } from '~/hooks/useExplorerStore';
import { TOP_BAR_HEIGHT } from '../../TopBar';
import AssignTagMenuItems from '../AssignTagMenuItems';
import FileThumb from '../File/Thumb';
import { getItemFilePath, getItemObject } from '../util';
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

interface Props extends Omit<ComponentProps<'div'>, 'onScroll'> {
	context?: ExplorerContext;
	data?: ExplorerItem;
}

export const Inspector = ({ data, context, ...elementProps }: Props) => {
	const objectData = data ? getItemObject(data) : null;
	const filePathData = data ? getItemFilePath(data) : null;
	const explorerStore = useExplorerStore();

	const isDir = data?.type === 'Path' ? data.item.is_dir : false;

	// this prevents the inspector from fetching data when the user is navigating quickly
	const [readyToFetch, setReadyToFetch] = useState(false);
	useEffect(() => {
		const timeout = setTimeout(() => {
			setReadyToFetch(true);
		}, 350);
		return () => clearTimeout(timeout);
	}, [data?.item.id]);

	// this is causing LAG
	const tags = useLibraryQuery(['tags.getForObject', objectData?.id || -1], {
		enabled: readyToFetch
	});

	const fullObjectData = useLibraryQuery(['files.get', { id: objectData?.id || -1 }], {
		enabled: readyToFetch && objectData?.id !== undefined
	});

	const item = data?.item;

	// map array of numbers into string
	const pub_id = fullObjectData?.data?.pub_id.map((n: number) => n.toString(16)).join('');

	return (
		<div
			{...elementProps}
			className="custom-scroll inspector-scroll h-screen w-full overflow-x-hidden pb-4 pl-1.5 pr-1"
			style={{ paddingTop: TOP_BAR_HEIGHT + 12 }}
		>
			{data && (
				<>
					{explorerStore.layoutMode !== 'media' && (
						<div
							className={clsx(
								'mb-[10px] flex h-[240] w-full items-center justify-center overflow-hidden'
							)}
						>
							<FileThumb loadOriginal size={240} data={data} />
						</div>
					)}
					<div className="flex w-full select-text flex-col overflow-hidden rounded-lg border border-app-line bg-app-box py-0.5 shadow-app-shade/10">
						<h3 className="truncate px-3 pb-1 pt-2 text-base font-bold">
							{filePathData?.name}
							{filePathData?.extension && `.${filePathData.extension}`}
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

						{context?.type == 'Location' && data?.type === 'Path' && (
							<MetaContainer>
								<MetaTitle>URI</MetaTitle>
								<MetaValue>{`${context.path}/${data.item.materialized_path}`}</MetaValue>
							</MetaContainer>
						)}
						<Divider />
						<MetaContainer>
							<div className="flex flex-wrap gap-1 overflow-hidden">
								<InfoPill>
									{isDir ? 'Folder' : ObjectKind[objectData?.kind || 0]}
								</InfoPill>
								{filePathData?.extension && (
									<InfoPill>{filePathData.extension}</InfoPill>
								)}
								{tags?.data?.map((tag) => (
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
								{objectData?.id && (
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
						</MetaContainer>
						<Divider />
						<MetaContainer className="!flex-row space-x-2">
							<MetaTextLine>
								<InspectorIcon component={Cube} />
								<span className="mr-1.5">Size</span>
								<MetaValue>
									{formatBytes(Number(filePathData?.size_in_bytes || 0))}
								</MetaValue>
							</MetaTextLine>
							{fullObjectData.data?.media_data?.duration_seconds && (
								<MetaTextLine>
									<InspectorIcon component={Clock} />
									<span className="mr-1.5">Duration</span>
									<MetaValue>
										{fullObjectData.data.media_data.duration_seconds}
									</MetaValue>
								</MetaTextLine>
							)}
						</MetaContainer>
						<Divider />
						<MetaContainer>
							<Tooltip label={dayjs(item?.date_created).format('h:mm:ss a')}>
								<MetaTextLine>
									<InspectorIcon component={Clock} />
									<MetaKeyName className="mr-1.5">Created</MetaKeyName>
									<MetaValue>
										{dayjs(item?.date_created).format('MMM Do YYYY')}
									</MetaValue>
								</MetaTextLine>
							</Tooltip>
							<Tooltip label={dayjs(item?.date_created).format('h:mm:ss a')}>
								<MetaTextLine>
									<InspectorIcon component={Barcode} />
									<MetaKeyName className="mr-1.5">Indexed</MetaKeyName>
									<MetaValue>
										{dayjs(filePathData?.date_indexed).format('MMM Do YYYY')}
									</MetaValue>
								</MetaTextLine>
							</Tooltip>
						</MetaContainer>

						{!isDir && objectData && (
							<>
								<Note data={objectData} />
								<Divider />
								<MetaContainer>
									<Tooltip label={filePathData?.cas_id || ''}>
										<MetaTextLine>
											<InspectorIcon component={Snowflake} />
											<MetaKeyName className="mr-1.5">Content ID</MetaKeyName>
											<MetaValue>{filePathData?.cas_id || ''}</MetaValue>
										</MetaTextLine>
									</Tooltip>
									{filePathData?.integrity_checksum && (
										<Tooltip label={filePathData?.integrity_checksum || ''}>
											<MetaTextLine>
												<InspectorIcon component={CircleWavyCheck} />
												<MetaKeyName className="mr-1.5">
													Checksum
												</MetaKeyName>
												<MetaValue>
													{filePathData?.integrity_checksum}
												</MetaValue>
											</MetaTextLine>
										</Tooltip>
									)}
									{pub_id && (
										<Tooltip label={pub_id || ''}>
											<MetaTextLine>
												<InspectorIcon component={Hash} />
												<MetaKeyName className="mr-1.5">
													Object ID
												</MetaKeyName>
												<MetaValue>{pub_id}</MetaValue>
											</MetaTextLine>
										</Tooltip>
									)}
								</MetaContainer>
							</>
						)}
					</div>
				</>
			)}
		</div>
	);
};
