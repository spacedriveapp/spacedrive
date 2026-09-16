import type {BaseLayoutProps} from 'fumadocs-ui/layouts/shared';
import Image from 'next/image';
import {appName, gitConfig} from './shared';

export function baseOptions(): BaseLayoutProps {
	return {
		nav: {
			title: (
				<>
					<Image
						src="/spacedrive-icon.webp"
						alt=""
						width={24}
						height={24}
					/>
					{appName}
				</>
			)
		},
		githubUrl: `https://github.com/${gitConfig.user}/${gitConfig.repo}`
	};
}
