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
}

export function Folder(props: FolderProps) {
	return (
		<img
			className={props.className}
			src={props.white ? folderWhiteSvg : folderSvg}
			alt="Folder icon"
		/>
	);
}
