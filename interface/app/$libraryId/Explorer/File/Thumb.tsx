import * as icons from '@sd/assets/icons';
import clsx from 'clsx';
import { CSSProperties, useEffect, useState } from 'react';
import { ExplorerItem, useLibraryContext } from '@sd/client';
import { useExplorerStore } from '~/hooks/useExplorerStore';
import { useIsDark, usePlatform } from '~/util/Platform';
import { getExplorerItemData } from '../util';
import classes from './Thumb.module.scss';

interface Props {
	data: ExplorerItem;
	size: number;
	loadOriginal?: boolean;
	className?: string;
	forceShowExtension?: boolean;
	extensionClassName?: string;
}

export default function Thumb(props: Props) {
	const { cas_id, isDir, kind, hasThumbnail, extension } = getExplorerItemData(props.data);
	const store = useExplorerStore();
	const platform = usePlatform();
	const { library } = useLibraryContext();
	const isDark = useIsDark();

	const [fullPreviewUrl, setFullPreviewUrl] = useState<string | null>(null);

	useEffect(() => {
		if (props.loadOriginal && hasThumbnail) {
			const url = platform.getFileUrl(library.uuid, store.locationId!, props.data.item.id);
			if (url) setFullPreviewUrl(url);
		}
	}, [
		props.data.item.id,
		hasThumbnail,
		library.uuid,
		props.loadOriginal,
		platform,
		store.locationId
	]);

	const videoBarsHeight = Math.floor(props.size / 10);
	const videoHeight = Math.floor((props.size * 9) / 16) + videoBarsHeight * 2;

	const imgStyle: CSSProperties =
		kind === 'Video'
			? {
					borderTopWidth: videoBarsHeight,
					borderBottomWidth: videoBarsHeight,
					width: props.size,
					height: videoHeight
			  }
			: {};

	let icon = icons['Document'];
	if (isDir) {
		icon = icons['Folder'];
	} else if (
		kind &&
		extension &&
		icons[`${kind}_${extension.toLowerCase()}` as keyof typeof icons]
	) {
		icon = icons[`${kind}_${extension.toLowerCase()}` as keyof typeof icons];
	} else if (kind !== 'Unknown' && kind && icons[kind as keyof typeof icons]) {
		icon = icons[kind as keyof typeof icons];
	}

	if (!hasThumbnail || !cas_id) {
		if (!isDark) {
			icon = icon?.substring(0, icon.length - 4) + '_Light' + '.png';
		}
		return <img src={icon} className={clsx('h-full overflow-hidden')} />;
	}

	return (
		<div
			className={clsx(
				'relative flex h-full shrink-0 items-center justify-center border-2 border-transparent',
				props.className
			)}
		>
			<img
				style={{ ...imgStyle, maxWidth: props.size, width: props.size - 10 }}
				decoding="async"
				className={clsx(
					'z-90 pointer-events-none',
					hasThumbnail &&
						'max-h-full w-auto max-w-full rounded-sm object-cover shadow shadow-black/30',
					kind === 'Image' && classes.checkers,
					kind === 'Image' && props.size > 60 && 'border-2 border-app-line',
					kind === 'Video' && 'rounded border-x-0 !border-black',
					props.className
				)}
				src={fullPreviewUrl || platform.getThumbnailUrlById(cas_id)}
			/>
			{extension &&
				kind === 'Video' &&
				hasThumbnail &&
				(props.size > 80 || props.forceShowExtension) && (
					<div
						className={clsx(
							'absolute bottom-[13%] right-[5%] rounded bg-black/60 py-0.5 px-1 text-[9px] font-semibold uppercase opacity-70',
							props.extensionClassName
						)}
					>
						{extension}
					</div>
				)}
		</div>
	);
}
