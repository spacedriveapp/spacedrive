import { ArrowCircleDown } from '@phosphor-icons/react';
import { ComponentProps } from 'react';
import { Platform } from '~/utils/current-platform';

import { CtaPrimaryButton } from './cta-primary-button';

interface DownloadButtonProps extends Omit<ComponentProps<'button'>, 'children'> {
	platform: Platform | null;
	shrinksOnSmallScreen?: boolean;
}

export function DownloadButton({
	platform,
	shrinksOnSmallScreen = false,
	...props
}: DownloadButtonProps) {
	const href = `https://spacedrive.com/api/releases/desktop/stable/${platform?.os ?? 'linux'}/x86_64`;
	const platformName = platform?.name === 'macOS' ? 'Mac' : platform?.name;

	return (
		<CtaPrimaryButton {...props} href={href} icon={<ArrowCircleDown weight="bold" size={22} />}>
			Download
			<span className={shrinksOnSmallScreen ? 'max-xl:hidden' : undefined}>
				{' '}
				for {platformName ?? 'Linux'}
			</span>
		</CtaPrimaryButton>
	);
}
