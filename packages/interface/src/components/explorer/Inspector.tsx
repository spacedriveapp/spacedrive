// import types from '../../constants/file-types.json';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { Barcode, CircleWavyCheck, Clock, Cube, Link, Lock, Snowflake } from 'phosphor-react';
import { useEffect, useState } from 'react';
import { ExplorerContext, ExplorerItem, useLibraryQuery } from '@sd/client';
import { Button, tw } from '@sd/ui';
import { ObjectKind } from '../../util/kind';
import { DefaultProps } from '../primitive/types';
import { Tooltip } from '../tooltip/Tooltip';
import FileThumb from './FileThumb';
import { Divider } from './inspector/Divider';
import FavoriteButton from './inspector/FavoriteButton';
import Note from './inspector/Note';
import { isObject } from './utils';

export const InfoPill = tw.span`inline border border-transparent px-1 text-[11px] font-medium shadow shadow-app-shade/5 bg-app-selected rounded-md text-ink-dull`;

export const PlaceholderPill = tw.span`inline border  px-1 text-[11px] shadow shadow-app-shade/10 rounded-md bg-transparent border-dashed border-app-active transition hover:text-ink-faint hover:border-ink-faint font-medium text-ink-faint/70`;

export const MetaContainer = tw.div`flex flex-col px-4 py-1.5`;

export const MetaTitle = tw.h5`text-xs font-bold`;

export const MetaValue = tw.p`text-xs break-all text-ink truncate`;

const MetaTextLine = tw.div`flex items-center my-0.5 text-xs text-ink-dull`;

const InspectorIcon = ({ component: Icon, ...props }: any) => (
	<Icon weight="bold" {...props} className={clsx('mr-2 flex-shrink-0', props.className)} />
);

interface Props extends DefaultProps<HTMLDivElement> {
	context?: ExplorerContext;
	data?: ExplorerItem;
}

export const Inspector = (props: Props) => {
	const { context, data, ...elementProps } = props;

	const is_dir = props.data?.type === 'Path' ? props.data.is_dir : false;

	const objectData = props.data ? (isObject(props.data) ? props.data : props.data.object) : null;
	const isDir = props.data?.type === 'Path' ? props.data.is_dir : false;

	// this prevents the inspector from fetching data when the user is navigating quickly
	const [readyToFetch, setReadyToFetch] = useState(false);
	useEffect(() => {
		const timeout = setTimeout(() => {
			setReadyToFetch(true);
		}, 350);
		return () => clearTimeout(timeout);
	}, [props.data?.id]);

	// this is causing LAG
	const tags = useLibraryQuery(['tags.getForObject', objectData?.id || -1], {
		enabled: readyToFetch
	});

	const fullObjectData = useLibraryQuery(['files.get', { id: objectData?.id || -1 }], {
		enabled: readyToFetch && objectData?.id !== undefined
	});

	return (
		<div
			{...elementProps}
			className="-mt-[50px] pt-[55px] z-10 pl-1.5 pr-1 w-full h-screen overflow-x-hidden custom-scroll inspector-scroll pb-4"
		>
			{!!props.data && (
				<>
					<div
						className={clsx(
							'flex h-52 items-center justify-center w-full mb-[10px] overflow-hidden rounded-lg',
							objectData?.kind === 7 && objectData?.has_thumbnail && 'bg-black'
						)}
					>
						<FileThumb
							iconClassNames="my-3 max-h-[150px]"
							size={230}
							kind={ObjectKind[objectData?.kind || 0]}
							className="flex flex-grow-0 flex-shrink bg-green-500"
							data={props.data}
						/>
					</div>
					<div className="flex flex-col w-full pt-0.5 pb-0.5 overflow-hidden bg-app-box rounded-lg select-text shadow-app-shade/10 border border-app-line">
						<h3 className="px-3 pt-2 pb-1 text-base font-bold truncate">
							{props.data?.name}
							{props.data?.extension && `.${props.data.extension}`}
						</h3>
						{objectData && (
							<div className="flex flex-row mt-1 mb-0.5 mx-3 space-x-0.5">
								<Tooltip label="Favorite">
									<FavoriteButton data={objectData} />
								</Tooltip>

								<Tooltip label="Encrypt">
									<Button size="icon">
										<Lock className="w-[18px] h-[18px]" />
									</Button>
								</Tooltip>
								<Tooltip label="Share">
									<Button size="icon">
										<Link className="w-[18px] h-[18px]" />
									</Button>
								</Tooltip>
							</div>
						)}

						{props.context?.type == 'Location' && props.data?.type === 'Path' && (
							<MetaContainer>
								<MetaTitle>URI</MetaTitle>
								<MetaValue>{`${props.context.local_path}/${props.data.materialized_path}`}</MetaValue>
							</MetaContainer>
						)}
						<Divider />
						{
							<MetaContainer>
								<div className="flex flex-wrap gap-1">
									<InfoPill>{isDir ? 'Folder' : ObjectKind[objectData?.kind || 0]}</InfoPill>
									{props.data.extension && <InfoPill>{props.data.extension}</InfoPill>}
									{tags?.data?.map((tag) => (
										<InfoPill
											className="!text-white"
											key={tag.id}
											style={{ backgroundColor: tag.color + 'CC' }}
										>
											{tag.name}
										</InfoPill>
									))}
									<PlaceholderPill>Add Tag</PlaceholderPill>
								</div>
							</MetaContainer>
						}
						<Divider />
						<MetaContainer className="!flex-row space-x-2">
							<MetaTextLine>
								<InspectorIcon component={Cube} />
								<span className="mr-1.5">Size</span>
								<MetaValue>{formatBytes(Number(objectData?.size_in_bytes || 0))}</MetaValue>
							</MetaTextLine>
							{fullObjectData.data?.media_data?.duration_seconds && (
								<MetaTextLine>
									<InspectorIcon component={Clock} />
									<span className="mr-1.5">Duration</span>
									<MetaValue>{fullObjectData.data.media_data.duration_seconds}</MetaValue>
								</MetaTextLine>
							)}
						</MetaContainer>
						<Divider />
						<MetaContainer>
							<Tooltip label={dayjs(props.data?.date_created).format('h:mm:ss a')}>
								<MetaTextLine>
									<InspectorIcon component={Clock} />
									<span className="mr-1.5">Created</span>
									<MetaValue>{dayjs(props.data?.date_created).format('MMM Do YYYY')}</MetaValue>
								</MetaTextLine>
							</Tooltip>
							<Tooltip label={dayjs(props.data?.date_created).format('h:mm:ss a')}>
								<MetaTextLine>
									<InspectorIcon component={Barcode} />
									<span className="mr-1.5">Indexed</span>
									<MetaValue>{dayjs(props.data?.date_indexed).format('MMM Do YYYY')}</MetaValue>
								</MetaTextLine>
							</Tooltip>
						</MetaContainer>

						{!is_dir && objectData && (
							<>
								<Note data={objectData} />
								<Divider />
								<MetaContainer>
									<Tooltip label={objectData?.cas_id || ''}>
										<MetaTextLine>
											<InspectorIcon component={Snowflake} />
											<span className="mr-1.5">Content ID</span>
											<MetaValue>{objectData?.cas_id || ''}</MetaValue>
										</MetaTextLine>
									</Tooltip>
									{objectData?.integrity_checksum && (
										<Tooltip label={objectData?.integrity_checksum || ''}>
											<MetaTextLine>
												<InspectorIcon component={CircleWavyCheck} />
												<span className="mr-1.5">Checksum</span>
												<MetaValue>{objectData.integrity_checksum}</MetaValue>
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

function formatBytes(bytes: number, decimals = 2) {
	if (bytes === 0) return '0 Bytes';

	const k = 1024;
	const dm = decimals < 0 ? 0 : decimals;
	const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB'];

	const i = Math.floor(Math.log(bytes) / Math.log(k));

	return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
}
