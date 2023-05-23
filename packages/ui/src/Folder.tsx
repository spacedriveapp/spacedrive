import { ReactComponent as folderWhiteSvg } from '@sd/assets/svgs/folder-white.svg';
import { ReactComponent as folderSvg } from '@sd/assets/svgs/folder.svg';

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

	const Icon = props.white ? folderWhiteSvg : folderSvg;

	return <Icon className={props.className} width={size} height={size} />;
}
