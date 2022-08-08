import { LocationContext } from '@sd/client';
import { FilePath } from '@sd/core';
import clsx from 'clsx';
import React, { useContext } from 'react';

import icons from '../../assets/icons';
import { ReactComponent as Folder } from '../../assets/svg/folder.svg';
import FileThumb from './FileThumb';

interface Props extends React.HTMLAttributes<HTMLDivElement> {
	file?: FilePath | null;
	selected?: boolean;
}

export default function FileItem(props: Props) {
	const location = useContext(LocationContext);

	return (
		<div {...props} className={clsx('inline-block w-[100px] mb-3', props.className)} draggable>
			<div
				className={clsx(
					'border-2 border-transparent rounded-lg text-center w-[100px] h-[100px] mb-1',
					{
						'bg-gray-50 dark:bg-gray-650': props.selected
					}
				)}
			>
				{props.file?.is_dir ? (
					<div className="flex items-center justify-center w-full h-full active:translate-y-[1px]">
						<div className="w-[70px]">
							<Folder className="" />
						</div>
					</div>
				) : props.file?.file?.has_thumbnail ? (
					<div
						className={clsx(
							'flex items-center justify-center h-full p-1 overflow-hidden rounded border-gray-550  shrink-0',
							props.selected && 'border-primary'
						)}
					>
						<div className="border-4 rounded border-gray-550">
							<FileThumb
								className="rounded-sm"
								file={props.file}
								locationId={location.location_id}
							/>
						</div>
					</div>
				) : (
					<div className="w-[64px] mt-1.5 m-auto transition duration-200 rounded-lg h-[90px] relative active:translate-y-[1px]">
						<svg
							className="absolute top-0 left-0 pointer-events-none fill-gray-150 dark:fill-gray-550"
							width="65"
							height="85"
							viewBox="0 0 65 81"
						>
							<path d="M0 8C0 3.58172 3.58172 0 8 0H39.6863C41.808 0 43.8429 0.842855 45.3431 2.34315L53.5 10.5L62.6569 19.6569C64.1571 21.1571 65 23.192 65 25.3137V73C65 77.4183 61.4183 81 57 81H8C3.58172 81 0 77.4183 0 73V8Z" />
						</svg>
						<svg
							width="22"
							height="22"
							className="absolute top-1 -right-[1px] z-0 fill-gray-50 dark:fill-gray-500 pointer-events-none"
							viewBox="0 0 41 41"
						>
							<path d="M41.4116 40.5577H11.234C5.02962 40.5577 0 35.5281 0 29.3238V0L41.4116 40.5577Z" />
						</svg>
						<div className="absolute flex flex-col items-center justify-center w-full h-full">
							{props.file?.extension && icons[props.file.extension as keyof typeof icons] ? (
								(() => {
									const Icon = icons[props.file.extension as keyof typeof icons];
									return (
										<Icon className="mt-2 pointer-events-none margin-auto w-[40px] h-[40px]" />
									);
								})()
							) : (
								<></>
							)}
							<span className="mt-1 text-xs font-bold text-center uppercase cursor-default text-gray-450">
								{props.file?.extension}
							</span>
						</div>
					</div>
				)}
			</div>
			<div className="flex justify-center">
				<span
					className={clsx(
						'px-1.5 py-[1px] truncate text-center rounded-md text-xs font-medium text-gray-550 dark:text-gray-300 cursor-default',
						{
							'bg-primary !text-white': props.selected
						}
					)}
				>
					{props.file?.name}
				</span>
			</div>
		</div>
	);
}
