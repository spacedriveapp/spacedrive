import { Folder, Folder_Light } from '@sd/assets/icons';
import { Image } from 'expo-image';

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
	return <Image source={isWhite ? Folder_Light : Folder} style={{ width: size, height: size }} />;
};

export default FolderIcon;
