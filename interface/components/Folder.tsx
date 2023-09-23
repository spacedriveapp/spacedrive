import {
	Database as Database_Dark,
	Database_Light,
	Folder as Folder_Dark,
	Folder_Light
} from '@sd/assets/icons';
import { useIsDark } from '~/hooks';

interface Props {
	/**
	 * Append additional classes to the underlying SVG
	 */
	className?: string;

	/**
	 * The size of the icon to show -- uniform width and height
	 */
	size?: number;
}

export function Folder({ size = 24, className }: Props) {
	const isDark = useIsDark();
	return (
		<img
			className={className}
			width={size}
			height={size}
			src={isDark ? Folder_Light : Folder_Dark}
			alt="Folder icon"
		/>
	);
}

export function Database({ size = 24, className }: Props) {
	const isDark = useIsDark();
	return (
		<img
			className={className}
			width={size}
			height={size}
			src={isDark ? Database_Light : Database_Dark}
			alt="Folder icon"
		/>
	);
}
