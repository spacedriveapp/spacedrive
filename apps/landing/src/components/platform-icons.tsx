import {
	faAndroid,
	faApple,
	faDocker,
	faLinux,
	faWindows
} from '@fortawesome/free-brands-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';

export function PlatformIcons() {
	return (
		<div className="mt-6 flex flex-wrap items-center justify-center gap-6">
			{/* macOS */}
			<FontAwesomeIcon
				icon={faApple}
				className="h-5 w-4 cursor-pointer text-gray-450 transition-colors hover:text-white"
			/>

			{/* Windows */}
			<FontAwesomeIcon
				icon={faWindows}
				className="h-5 w-5 cursor-pointer text-gray-450 transition-colors hover:text-white"
			/>

			{/* Linux */}
			<FontAwesomeIcon
				icon={faLinux}
				className="h-5 w-5 cursor-pointer text-gray-450 transition-colors hover:text-white"
			/>

			{/* Android */}
			<FontAwesomeIcon
				icon={faAndroid}
				className="h-5 w-5 cursor-pointer text-gray-450 transition-colors hover:text-white"
			/>

			{/* Docker */}
			<FontAwesomeIcon
				icon={faDocker}
				className="h-5 w-5 cursor-pointer text-gray-450 transition-colors hover:text-white"
			/>
		</div>
	);
}
