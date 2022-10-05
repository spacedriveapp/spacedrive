// import types from '../../constants/file-types.json';
import { Tooltip } from '../tooltip/Tooltip';
import FileThumb from './FileThumb';
import { Divider } from './inspector/Divider';
import FavoriteButton from './inspector/FavoriteButton';
import { MetaItem } from './inspector/MetaItem';
import Note from './inspector/Note';
import { isObject } from './utils';
import { ShareIcon } from '@heroicons/react/24/solid';
import { useLibraryQuery } from '@sd/client';
import { ExplorerContext, ExplorerItem, FilePath, Object } from '@sd/client';
import { Button } from '@sd/ui';
import { useQuery } from '@tanstack/react-query';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { Link } from 'phosphor-react';
import { useEffect, useState } from 'react';

interface Props {
	context?: ExplorerContext;
	data: ExplorerItem;
}

export const Inspector = (props: Props) => {
	const { data: types } = useQuery(
		['_file-types'],
		() => import('../../constants/file-types.json')
	);

	const is_dir = props.data?.type === 'Path' ? props.data.is_dir : false;

	const objectData = isObject(props.data) ? props.data : props.data.object;

	// this prevents the inspector from fetching data when the user is navigating quickly
	const [readyToFetch, setReadyToFetch] = useState(false);
	useEffect(() => {
		const timeout = setTimeout(() => {
			setReadyToFetch(true);
		}, 350);
		return () => clearTimeout(timeout);
	}, [props.data.id]);

	// this is causing LAG
	const { data: tags } = useLibraryQuery(['tags.getForObject', objectData?.id || -1], {
		enabled: readyToFetch
	});

	return (
		<div className="p-2 pr-1 overflow-x-hidden custom-scroll inspector-scroll pb-[55px]">
			{!!props.data && (
				<>
					<div className="flex bg-black items-center justify-center w-full h-64 mb-[10px] overflow-hidden rounded-lg ">
						<FileThumb size={230} className="!m-0 flex flex-shrink flex-grow-0" data={props.data} />
					</div>
					<div className="flex flex-col w-full pt-0.5 pb-4 overflow-hidden bg-white rounded-lg shadow select-text dark:shadow-gray-700 dark:bg-gray-550 dark:bg-opacity-40">
						<h3 className="pt-3 pl-3 text-base font-bold">
							{props.data?.name}
							{props.data?.extension && `.${props.data.extension}`}
						</h3>
						{objectData && (
							<div className="flex flex-row m-3 space-x-2">
								<Tooltip label="Favorite">
									<FavoriteButton data={objectData} />
								</Tooltip>
								<Tooltip label="Share">
									<Button size="sm" noPadding>
										<ShareIcon className="w-[18px] h-[18px]" />
									</Button>
								</Tooltip>
								<Tooltip label="Link">
									<Button size="sm" noPadding>
										<Link className="w-[18px] h-[18px]" />
									</Button>
								</Tooltip>
							</div>
						)}
						{!!tags?.length && (
							<>
								<Divider />
								<MetaItem
									// title="Tags"
									value={
										<div className="flex flex-wrap mt-1.5 gap-1.5">
											{tags?.map((tag) => (
												<div
													// onClick={() => setSelectedTag(tag.id === selectedTag ? null : tag.id)}
													key={tag.id}
													className={clsx(
														'flex items-center rounded px-1.5 py-0.5'
														// selectedTag === tag.id && 'ring'
													)}
													style={{ backgroundColor: tag.color + 'CC' }}
												>
													<span className="text-xs text-white drop-shadow-md">{tag.name}</span>
												</div>
											))}
										</div>
									}
								/>
							</>
						)}
						{props.context?.type == 'Location' && props.data?.type === 'Path' && (
							<>
								<Divider />
								<MetaItem
									title="URI"
									value={`${props.context.local_path}/${props.data.materialized_path}`}
								/>
							</>
						)}
						<Divider />
						<MetaItem
							title="Date Created"
							value={dayjs(props.data?.date_created).format('MMMM Do YYYY, h:mm:ss a')}
						/>
						<Divider />
						<MetaItem
							title="Date Indexed"
							value={dayjs(props.data?.date_indexed).format('MMMM Do YYYY, h:mm:ss a')}
						/>
						{!is_dir && (
							<>
								<Divider />
								<div className="flex flex-row items-center px-3 py-2 meta-item">
									{props.data?.extension && (
										<span className="inline px-1 mr-1 text-xs font-bold uppercase bg-gray-500 rounded-md text-gray-150">
											{props.data?.extension}
										</span>
									)}
									<p className="text-xs text-gray-600 break-all truncate dark:text-gray-300">
										{props.data?.extension && types !== undefined
											? //@ts-ignore
											  types[props.data.extension.toUpperCase()]?.descriptions.join(' / ')
											: 'Unknown'}
									</p>
								</div>
								{objectData && (
									<>
										<Note data={objectData} />
										<Divider />
										{objectData.cas_id && (
											<MetaItem title="Unique Content ID" value={objectData.cas_id} />
										)}
									</>
								)}
							</>
						)}
					</div>
				</>
			)}
		</div>
	);
};
