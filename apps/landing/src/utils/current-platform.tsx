'use client';

import { AndroidLogo, Globe, Icon, LinuxLogo, WindowsLogo } from '@phosphor-icons/react';
import { Apple, Docker } from '@sd/assets/svgs/brands';
import { ComponentProps, FunctionComponent, useEffect, useState } from 'react';
import { Tooltip } from '@sd/ui';

export type Platform = {
	name: string;
	os?: string;
	icon: React.ElementType<any> | Icon;
	version?: string;
	links?: Array<{ name: string; arch: string }>;
	disabled?: boolean;
	note?: string;
};

export const platforms = {
	darwin: {
		name: 'macOS',
		os: 'darwin',
		icon: Apple,
		version: '12+',
		links: [
			{ name: 'Intel', arch: 'x86_64' },
			{ name: 'Apple silicon', arch: 'aarch64' }
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
		links: [{ name: 'x86_64', arch: 'x86_64' }],
		note: 'Supports Ubuntu 22.04+, Debian Bookworm+, Linux Mint 21+, PopOS 22.04+'
	},
	docker: { name: 'Docker', icon: Docker },
	android: { name: 'Android', icon: AndroidLogo, version: '10+', disabled: true },
	web: { name: 'Web', icon: Globe, disabled: true }
} satisfies Record<string, Platform>;

export async function getCurrentPlatform(): Promise<Platform | null> {
	const { isWindows, isMacOs, isMobile } = await import('react-device-detect');

	if (isWindows) {
		return platforms.windows;
	} else if (isMacOs) {
		return platforms.darwin;
	} else if (!isMobile) {
		return platforms.linux;
	}

	return null;
}

export function useCurrentPlatform() {
	const [currentPlatform, setCurrentPlatform] = useState<Platform | null>(null);

	useEffect(() => {
		if (currentPlatform) return;

		getCurrentPlatform().then((platform) => {
			setCurrentPlatform((existingPlatform) => {
				if (existingPlatform) return existingPlatform;
				return platform;
			});
		});
	}, []);

	return currentPlatform;
}

export const BASE_DL_LINK = '/api/releases/desktop/stable';
