import { ShareIcon } from '@heroicons/react/solid';
import { useLibraryMutation } from '@sd/client';
import { FilePath, Location } from '@sd/core';
import { Button, TextArea } from '@sd/ui';
import moment from 'moment';
import { Heart, Link } from 'phosphor-react';
import React, { useEffect, useState } from 'react';

import types from '../../constants/file-types.json';
import FileThumb from './FileThumb';

interface MetaItemProps {
	title: string;
	value: string | React.ReactNode;
}

const MetaItem = (props: MetaItemProps) => {
	return (
		<div data-tip={props.value} className="flex flex-col px-3 py-1 meta-item">
			<h5 className="text-xs font-bold">{props.title}</h5>
			{typeof props.value === 'string' ? (
				<p className="text-xs text-gray-600 break-all truncate dark:text-gray-300">{props.value}</p>
			) : (
				props.value
			)}
		</div>
	);
};

const Divider = () => <div className="w-full my-1 h-[1px] bg-gray-100 dark:bg-gray-550" />;

export const Inspector = (props: {
	locationId: number;
	location?: Location | null;
	selectedFile?: FilePath;
}) => {
	const file_path = props.selectedFile,
		file_id = props.selectedFile?.file?.id || -1;

	const [favorite, setFavorite] = useState(false);
	const { mutate: fileToggleFavorite, isLoading: isFavoriteLoading } = useLibraryMutation(
		'files.setFavorite',
		{
			onError: () => setFavorite(!!props.selectedFile?.file?.favorite)
		}
	);
	const { mutate: fileSetNote } = useLibraryMutation('files.setNote');

	// notes are cached in a store by their file id
	// this is so we can ensure every note has been sent to Rust even
	// when quickly navigating files, which cancels update function
	const [note, setNote] = useState(props.location?.local_path || '');
	useEffect(() => {
		// Update debounced value after delay
		const handler = setTimeout(() => {
			fileSetNote({
				id: file_id,
				note
			});
		}, 500);

		return () => {
			clearTimeout(handler);
		};
	}, [note]);

	const toggleFavorite = () => {
		if (!isFavoriteLoading) {
			fileToggleFavorite({ id: file_id, favorite: !favorite });
			setFavorite(!favorite);
		}
	};

	useEffect(() => {
		setFavorite(!!props.selectedFile?.file?.favorite);
	}, [props.selectedFile]);

	// when input is updated, cache note
	function handleNoteUpdate(e: React.ChangeEvent<HTMLTextAreaElement>) {
		if (e.target.value !== note) {
			setNote(e.target.value);
		}
	}

	return (
		<div className="p-2 pr-1 w-[330px] overflow-x-hidden custom-scroll inspector-scroll pb-[55px]">
			{!!file_path && (
				<div>
					<div className="flex items-center justify-center w-full h-64 mb-[10px] overflow-hidden rounded-lg bg-gray-50 dark:bg-gray-900">
						<FileThumb
							className="!m-0 flex flex-shrink flex-grow-0"
							file={file_path}
							locationId={props.locationId}
						/>
					</div>
					<div className="flex flex-col w-full pb-2 overflow-hidden bg-white rounded-lg select-text dark:bg-gray-550 dark:bg-opacity-40">
						<h3 className="pt-3 pl-3 text-base font-bold">{file_path?.name}</h3>
						<div className="flex flex-row m-3 space-x-2">
							<Button onClick={toggleFavorite} size="sm" noPadding>
								<Heart weight={favorite ? 'fill' : 'regular'} className="w-[18px] h-[18px]" />
							</Button>
							<Button size="sm" noPadding>
								<ShareIcon className="w-[18px] h-[18px]" />
							</Button>
							<Button size="sm" noPadding>
								<Link className="w-[18px] h-[18px]" />
							</Button>
						</div>
						{file_path?.file?.cas_id && (
							<MetaItem title="Unique Content ID" value={file_path.file.cas_id as string} />
						)}
						<Divider />
						<MetaItem
							title="URI"
							value={`${props.location?.local_path}/${file_path?.materialized_path}`}
						/>
						<Divider />
						<MetaItem
							title="Date Created"
							value={moment(file_path?.date_created).format('MMMM Do YYYY, h:mm:ss a')}
						/>
						<Divider />
						<MetaItem
							title="Date Indexed"
							value={moment(file_path?.date_indexed).format('MMMM Do YYYY, h:mm:ss a')}
						/>
						{!file_path?.is_dir && (
							<>
								<Divider />
								<div className="flex flex-row items-center px-3 py-2 meta-item">
									{file_path?.extension && (
										<span className="inline px-1 mr-1 text-xs font-bold uppercase bg-gray-500 rounded-md text-gray-150">
											{file_path?.extension}
										</span>
									)}
									<p className="text-xs text-gray-600 break-all truncate dark:text-gray-300">
										{file_path?.extension
											? //@ts-ignore
											  types[file_path.extension.toUpperCase()]?.descriptions.join(' / ')
											: 'Unknown'}
									</p>
								</div>
								{file_path.file && (
									<>
										<Divider />
										<MetaItem
											title="Note"
											value={
												<TextArea
													className="mt-2 text-xs leading-snug !py-2"
													value={note || ''}
													onChange={handleNoteUpdate}
												/>
											}
										/>
									</>
								)}
							</>
						)}
					</div>
				</div>
			)}
		</div>
	);
};
