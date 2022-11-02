import FolderWhite from '@sd/assets/svgs/folder-white.svg';
import Folder from '@sd/assets/svgs/folder.svg';
import { SvgProps } from 'react-native-svg';

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
