'use client';

import { Github } from '@sd/assets/svgs/brands';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import { usePlausible } from 'next-plausible';
import { useState } from 'react';

import HomeCTA from '../HomeCTA';
import { DockerDialog } from './DockerDialog';
import { BASE_DL_LINK, Platform, platforms, useCurrentPlatform } from './Platform';

interface Props {
	latestVersion: string;
}

export function Downloads({ latestVersion }: Props) {
	const [selectedPlatform, setSelectedPlatform] = useState<Platform | null>(null);
	const currentPlatform = useCurrentPlatform();

	const [dockerDialogOpen, setDockerDialogOpen] = useState(false);

	const plausible = usePlausible();

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
								className={`z-5 relative`}
								icon={Icon ? <Icon width="1rem" height="1rem" /> : undefined}
								text={`Download for ${currentPlatform.name}`}
								onClick={() => {
									plausible('download', {
										props: { os: currentPlatform.name }
									});
									setSelectedPlatform(currentPlatform);
								}}
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
							rel="noopener"
							href={`${BASE_DL_LINK}/${selectedPlatform.os}/${arch}`}
							onClick={() => {
								plausible('download', {
									props: { os: selectedPlatform.name + ' ' + arch }
								});
							}}
							className="z-5 relative !py-1 !text-sm"
						/>
					))}
				</div>
			)}
			<p className="animation-delay-3 z-30 mt-3 px-6 text-center text-sm text-gray-400 fade-in">
				{latestVersion}
				{formattedVersion && (
					<>
						<span className="mx-2 opacity-50">|</span>
						{formattedVersion}
					</>
				)}
			</p>
			{/* Platform icons */}
			<div className="relative z-10 mt-5 flex gap-3">
				{Object.values<Platform>(platforms).map((platform, i) => {
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
								className={clsx(platform.name === 'Docker' && 'cursor-pointer')}
								onClick={() => {
									if (platform.name === 'Docker') {
										setDockerDialogOpen(true);
										return;
									}
									if (platform.links) {
										if (platform.links.length === 1) {
											plausible('download', {
												props: { os: platform.name }
											});
										} else {
											setSelectedPlatform(platform);
										}
									}
								}}
							/>
						</motion.div>
					);
				})}
			</div>
			{/* Docker Dialog */}
			<DockerDialog open={dockerDialogOpen} setOpen={setDockerDialogOpen} />
		</>
	);
}
