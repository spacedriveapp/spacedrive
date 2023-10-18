'use client';

import { AndroidLogo, Globe, LinuxLogo, WindowsLogo } from '@phosphor-icons/react/dist/ssr';
import { Apple, Github } from '@sd/assets/svgs/brands';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import { ComponentProps, FunctionComponent, useEffect, useState } from 'react';
import { Tooltip } from '@sd/ui';

import HomeCTA from './HomeCTA';

const RELEASE_VERSION = 'Alpha v0.1.1';

interface Platform {
	name: string;
	os?: string;
	icon: FunctionComponent<any>;
	version?: string;
	links?: Array<{ name: string; arch: string }>;
}

const platforms = {
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
		version: 'AppImage',
		links: [{ name: 'x86_64', arch: 'x86_64' }]
	},
	android: { name: 'Android', icon: AndroidLogo, version: '10+' },
	web: { name: 'Web', icon: Globe }
} satisfies Record<string, Platform>;

const BASE_DL_LINK = '/api/releases/desktop/stable';

export function Downloads() {
	const [selectedPlatform, setSelectedPlatform] = useState<Platform | null>(null);
	const currentPlatform = useCurrentPlatform();

	const formattedVersion = (() => {
		const platform = selectedPlatform ?? currentPlatform;

		if (!platform?.version) return;
		if (platform.name === 'Linux') return platform.version;

		return `${platform.name} ${platform.version}`;
	})();

	return (
		<>
			<div className="flex flex-row gap-3">
				{currentPlatform &&
					(() => {
						const Icon = currentPlatform.icon;
						const { links } = currentPlatform;

						return (
							<HomeCTA
								href={
									links?.length === 1
										? `${BASE_DL_LINK}/${currentPlatform.os}/${links[0].arch}`
										: undefined
								}
								className={`z-5 plausible-event-name=download relative plausible-event-os=${currentPlatform.name}`}
								icon={Icon ? <Icon width="1rem" height="1rem" /> : undefined}
								text={`Download for ${currentPlatform.name}`}
								onClick={() => setSelectedPlatform(currentPlatform)}
							/>
						);
					})()}

				<HomeCTA
					target="_blank"
					href="https://www.github.com/spacedriveapp/spacedrive"
					icon={<Github />}
					className="z-5 relative"
					text="Star on GitHub"
				/>
			</div>

			{selectedPlatform?.links && selectedPlatform.links.length > 1 && (
				<div className="z-50 mb-2 mt-4 flex flex-row gap-3 fade-in">
					{selectedPlatform.links.map(({ name, arch }) => (
						<HomeCTA
							key={name}
							size="md"
							text={name}
							target="_blank"
							href={`${BASE_DL_LINK}/${selectedPlatform.os}/${arch}`}
							className={clsx(
								'z-5 relative !py-1 !text-sm',
								`plausible-event-name=download plausible-event-os=${selectedPlatform.name}+${arch}`
							)}
						/>
					))}
				</div>
			)}
			<p className="animation-delay-3 z-30 mt-3 px-6 text-center text-sm text-gray-400 fade-in">
				{RELEASE_VERSION}
				{formattedVersion && (
					<>
						<span className="mx-2 opacity-50">|</span>
						{formattedVersion}
					</>
				)}
			</p>
			<div className="relative z-10 mt-5 flex gap-3">
				{Object.values(platforms as Record<string, Platform>).map((platform, i) => {
					return (
						<motion.div
							initial={{ opacity: 0, y: 20 }}
							animate={{ opacity: 1, y: 0 }}
							transition={{ delay: i * 0.2, ease: 'easeInOut' }}
							key={platform.name}
						>
							<Platform
								key={platform.name}
								platform={platform}
								className={clsx(
									platform.links?.length == 1 &&
										`plausible-event-name=download plausible-event-os=${platform.name}`
								)}
								onClick={() => {
									if (platform.links && platform.links.length > 1)
										setSelectedPlatform(platform);
								}}
							/>
						</motion.div>
					);
				})}
			</div>
		</>
	);
}

function useCurrentPlatform() {
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

interface Props {
	platform: Platform;
}
function Platform({ platform, ...props }: ComponentProps<'a'> & Props) {
	const { links } = platform;

	const Outer = links
		? links.length === 1
			? (props: any) => (
					<a
						aria-label={platform.name}
						target="_blank"
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
					size={25}
					className={`h-[25px] text-white ${
						platform.links ? 'opacity-80' : 'opacity-20'
					}`}
					weight="fill"
				/>
			</Outer>
		</Tooltip>
	);
}
