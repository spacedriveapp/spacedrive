import React from 'react';

import folderWhiteSvg from '../../assets/svg/folder-white.svg';
import folderSvg from '../../assets/svg/folder.svg';

interface FolderProps {
	/**
	 * Append additional classes to the underlying SVG
	 */
	className?: string;

	/**
	 * Render a white folder icon
	 */
	white?: boolean;

	/**
	 * The size of the icon to show -- uniform width and height
	 */
	size?: number;
}

export function Folder(props: FolderProps) {
	const { size = 24 } = props;

	return (
		<img
			className={props.className}
			width={size}
			height={size}
			src={props.white ? folderWhiteSvg : folderSvg}
			alt="Folder icon"
		/>
	);
}
