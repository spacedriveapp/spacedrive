import { explorerStore, usePlatform } from '@sd/client';
import { ExplorerItem } from '@sd/core';
import clsx from 'clsx';
import React, { useContext, useState } from 'react';
import { useSnapshot } from 'valtio';

import icons from '../../assets/icons';
import { Folder } from '../icons/Folder';
import { isObject, isPath } from './utils';

interface Props {
	data: ExplorerItem;
	size: number;
	className?: string;
	style?: React.CSSProperties;
}

export default function FileThumb({ data, ...props }: Props) {
	const platform = usePlatform();
	const store = useSnapshot(explorerStore);

	if (isPath(data) && data.is_dir) return <Folder size={props.size * 0.7} />;

	const cas_id = isObject(data) ? data.cas_id : data.file?.cas_id;

	if (!cas_id) return <div></div>;

	const has_thumbnail = isObject(data)
		? data.has_thumbnail
		: isPath(data)
		? data.file?.has_thumbnail
		: !!store.newThumbnails[cas_id];

	if (has_thumbnail)
		return (
			<img
				// onLoad={}
				style={props.style}
				className={clsx('pointer-events-none z-90', props.className)}
				src={platform.getThumbnailUrlById(cas_id)}
			/>
		);

	const Icon = icons[data.extension as keyof typeof icons];

	return (
		<div
			style={{ width: props.size * 0.8, height: props.size * 0.8 }}
			className="relative m-auto transition duration-200 "
		>
			<svg
				// BACKGROUND
				className="absolute -translate-x-1/2 -translate-y-1/2 pointer-events-none top-1/2 left-1/2 fill-gray-150 dark:fill-gray-550"
				width="100%"
				height="100%"
				viewBox="0 0 65 81"
				style={{ filter: 'drop-shadow(0px 5px 2px rgb(0 0 0 / 0.05))' }}
			>
				<path d="M0 8C0 3.58172 3.58172 0 8 0H39.6863C41.808 0 43.8429 0.842855 45.3431 2.34315L53.5 10.5L62.6569 19.6569C64.1571 21.1571 65 23.192 65 25.3137V73C65 77.4183 61.4183 81 57 81H8C3.58172 81 0 77.4183 0 73V8Z" />
			</svg>
			{Icon && (
				<div className="absolute flex flex-col items-center justify-center w-full h-full mt-0.5 ">
					<Icon
						className={clsx('w-full h-full ')}
						style={{ width: props.size * 0.45, height: props.size * 0.45 }}
					/>
					<span className="text-xs font-bold text-center uppercase cursor-default text-gray-450">
						{data.extension}
					</span>
				</div>
			)}
			<svg
				// PEEL
				width="28%"
				height="28%"
				className="absolute top-0 right-0 -translate-x-[35%] z-0 pointer-events-none fill-gray-50 dark:fill-gray-500"
				viewBox="0 0 41 41"
			>
				<path d="M41.4116 40.5577H11.234C5.02962 40.5577 0 35.5281 0 29.3238V0L41.4116 40.5577Z" />
			</svg>
		</div>
	);

	return null;
}
