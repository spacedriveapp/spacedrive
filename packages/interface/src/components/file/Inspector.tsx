import { Transition } from '@headlessui/react';
import { ShareIcon } from '@heroicons/react/solid';
import { useInspectorStore } from '@sd/client';
import { FilePath, LocationResource } from '@sd/core';
import { Button, TextArea } from '@sd/ui';
import moment from 'moment';
import { Heart, Link } from 'phosphor-react';
import React, { useEffect } from 'react';

import { default as types } from '../../constants/file-types.json';
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
	location?: LocationResource;
	selectedFile?: FilePath;
}) => {
	const file_path = props.selectedFile,
		full_path = `${props.location?.path}/${file_path?.materialized_path}`,
		file_id = props.selectedFile?.file?.id || -1;

	// notes are cached in a store by their file id
	// this is so we can ensure every note has been sent to Rust even
	// when quickly navigating files, which cancels update function
	const { notes, setNote, unCacheNote } = useInspectorStore();

	// show cached note over server note, important to check for undefined not falsey
	const note =
		notes[file_id] === undefined ? props.selectedFile?.file?.note || null : notes[file_id];

	// when input is updated, cache note
	function handleNoteUpdate(e: React.ChangeEvent<HTMLTextAreaElement>) {
		if (e.target.value !== note) {
			setNote(file_id, e.target.value);
		}
	}

	useEffect(() => {
		// if the notes are synced, remove cache
		if (notes[file_id] === props.selectedFile?.file?.note) {
			unCacheNote(file_id);
		}
	}, [note]);

	return (
		// <Transition
		// 	as={React.Fragment}
		// 	show={true}
		// 	enter="transition-translate ease-in-out duration-200"
		// 	enterFrom="translate-x-64"
		// 	enterTo="translate-x-0"
		// 	leave="transition-translate ease-in-out duration-200"
		// 	leaveFrom="translate-x-0"
		// 	leaveTo="translate-x-64"
		// >
		<div className="flex p-2 pr-1 mr-1 pb-[51px]  w-[330px] flex-wrap overflow-x-hidden custom-scroll inspector-scroll">
			{!!file_path && (
				<div className="flex flex-col w-full pb-2 overflow-hidden bg-white rounded-lg select-text dark:bg-gray-600 bg-opacity-70">
					<div className="flex items-center justify-center w-full h-64 overflow-hidden rounded-t-lg bg-gray-50 dark:bg-gray-900">
						<FileThumb
							hasThumbnailOverride={false}
							className="!m-0 flex flex-shrink flex-grow-0"
							file={file_path}
							locationId={props.locationId}
						/>
					</div>
					<h3 className="pt-3 pl-3 text-base font-bold">{file_path?.name}</h3>
					<div className="flex flex-row m-3 space-x-2">
						<Button size="sm" noPadding>
							<Heart className="w-[18px] h-[18px]" />
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
					<MetaItem title="URI" value={full_path} />
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
					{/* <div className="flex flex-row m-3">
              <Button size="sm">Mint</Button>
            </div> */}
					{/* <MetaItem title="Date Last Modified" value={file?.date_modified} />
            <MetaItem title="Date Indexed" value={file?.date_indexed} /> */}
				</div>
			)}
		</div>
		// </Transition>
	);
};
