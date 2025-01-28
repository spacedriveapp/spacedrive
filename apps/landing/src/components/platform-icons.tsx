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
		<div className="relative z-40 mt-6 flex flex-wrap items-center justify-center gap-6">
			{/* macOS */}
			<a
				href="https://spacedrive.com/api/releases/desktop/stable/darwin/aarch64"
				className="group relative"
			>
				<FontAwesomeIcon
					icon={faApple}
					className="h-5 w-4 cursor-pointer text-white/50 transition-colors hover:text-white"
				/>
			</a>

			{/* Windows */}
			<a
				href="https://spacedrive.com/api/releases/desktop/stable/windows/x86_64"
				className="group relative"
			>
				<FontAwesomeIcon
					icon={faWindows}
					className="size-5 cursor-pointer text-white/50 transition-colors hover:text-white"
				/>
			</a>

			{/* Linux */}
			<a
				href="https://spacedrive.com/api/releases/desktop/stable/linux/x86_64"
				className="group relative"
			>
				<FontAwesomeIcon
					icon={faLinux}
					className="size-5 cursor-pointer text-white/50 transition-colors hover:text-white"
				/>
			</a>

			{/* Android - Coming soon */}
			<span className="w-fit cursor-not-allowed">
				<FontAwesomeIcon
					icon={faAndroid}
					className="size-5 cursor-not-allowed text-white/30"
				/>
			</span>

			{/* Docker */}
			<a
				href="https://github.com/spacedriveapp/spacedrive/pkgs/container/spacedrive%2Fserver"
				className="group relative"
			>
				<FontAwesomeIcon
					icon={faDocker}
					className="size-5 cursor-pointer text-white/50 transition-colors hover:text-white"
				/>
			</a>
		</div>
	);
}
