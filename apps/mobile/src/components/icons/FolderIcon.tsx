import FolderWhite from '@sd/assets/svgs/folder-white.svg';
import { Image } from 'react-native';

type FolderProps = {
	/**
	 * Render a white folder icon
	 */
	isWhite?: boolean;

	/**
	 * The size of the icon to show -- uniform width and height
	 */
	size?: number;
};

const FolderIcon: React.FC<FolderProps> = ({ size = 24, isWhite }) => {
	return isWhite ? (
		<FolderWhite width={size} height={size} />
	) : (
		<Image source={require('@sd/assets/images/Folder.png')} style={{ width: size, height: size }} />
	);
};

export default FolderIcon;
