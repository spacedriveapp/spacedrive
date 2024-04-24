'use client';

import { AndroidLogo, Globe, Icon, LinuxLogo, WindowsLogo } from '@phosphor-icons/react';
import { Apple, Docker } from '@sd/assets/svgs/brands';
import { ComponentProps, FunctionComponent, useEffect, useState } from 'react';
import { Tooltip } from '@sd/ui';

export type Platform = {
	name: string;
	os?: string;
	icon: Icon | FunctionComponent<any>;
	version?: string;
	links?: Array<{ name: string; arch: string }>;
	disabled?: boolean;
};

export const platforms = {
	darwin: {
		name: 'macOS',
		os: 'darwin',
		icon: Apple,
		version: '12+',
		links: [
			{ name: 'Intel', arch: 'x86_64' },
			{ name: 'Apple Silicon', arch: 'aarch64' }
		]
	},
	windows: {
		name: 'Windows',
		os: 'windows',
		icon: WindowsLogo,
		version: '10+',
		links: [{ name: 'x86_64', arch: 'x86_64' }]
	},
	linux: {
		name: 'Linux',
		os: 'linux',
		icon: LinuxLogo,
		version: 'deb',
		links: [{ name: 'x86_64', arch: 'x86_64' }]
	},
	docker: { name: 'Docker', icon: Docker },
	android: { name: 'Android', icon: AndroidLogo, version: '10+', disabled: true },
	web: { name: 'Web', icon: Globe, disabled: true }
} satisfies Record<string, Platform>;

export function useCurrentPlatform() {
	const [currentPlatform, setCurrentPlatform] = useState<Platform | null>(null);

	useEffect(() => {
		import('react-device-detect').then(({ isWindows, isMacOs, isMobile }) => {
			setCurrentPlatform((e) => {
				if (e) return e;

				if (isWindows) {
					return platforms.windows;
				} else if (isMacOs) {
					return platforms.darwin;
				} else if (!isMobile) {
					return platforms.linux;
				}

				return null;
			});
		});
	}, []);

	return currentPlatform;
}

interface PlatformProps {
	platform: Platform;
}

export const BASE_DL_LINK = '/api/releases/desktop/stable';

export function Platform({ platform, ...props }: ComponentProps<'a'> & PlatformProps) {
	const { links } = platform;

	const Outer = links
		? links.length === 1
			? (props: any) => (
					<a
						aria-label={platform.name}
						rel="noopener"
						href={`${BASE_DL_LINK}/${platform.os}/${links[0].arch}`}
						{...props}
					/>
				)
			: (props: any) => <button {...props} />
		: (props: any) => <div {...props} />;

	const Icon = platform.icon;

	return (
		<Tooltip label={platform.name}>
			<Outer {...props}>
				<Icon
					className={`size-[24px] text-white ${
						platform.disabled ? 'opacity-20' : 'opacity-90'
					}`}
					weight="fill"
				/>
			</Outer>
		</Tooltip>
	);
}
