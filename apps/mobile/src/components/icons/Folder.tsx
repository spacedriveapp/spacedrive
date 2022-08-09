import React from 'react';
import { SvgProps } from 'react-native-svg';

import FolderWhite from '../../assets/temp/folder-white.svg';
import Folder from '../../assets/temp/folder.svg';

type FolderProps = {
	/**
	 * Render a white folder icon
	 */
	isWhite?: boolean;

	/**
	 * The size of the icon to show -- uniform width and height
	 */
	size?: number;
} & SvgProps;

const FolderIcon: React.FC<FolderProps> = ({ size = 24, isWhite, ...svgProps }) => {
	return isWhite ? (
		<FolderWhite width={size} height={size} {...svgProps} />
	) : (
		<Folder width={size} height={size} {...svgProps} />
	);
};

export default FolderIcon;
