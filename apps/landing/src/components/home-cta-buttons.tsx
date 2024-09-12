'use client';

import { useState } from 'react';
import { CtaPrimaryButton } from '~/components/cta-primary-button';
import { CtaSecondaryButton } from '~/components/cta-secondary-button';

import { useCurrentPlatform, type Platform } from '../utils/current-platform';

interface Props {
	latestVersion: string;
}

export function HomeCtaButtons({ latestVersion }: Props) {
	const [selectedPlatform, setSelectedPlatform] = useState<Platform | null>(null);
	const currentPlatform = useCurrentPlatform();

	const [dockerDialogOpen, setDockerDialogOpen] = useState(false);

	const [formattedVersion, note] = (() => {
		const platform = selectedPlatform ?? currentPlatform;
		return platform
			? [
					platform.version &&
						(platform.name === 'Linux'
							? platform.version
							: `${platform.name} ${platform.version}`),
					platform.note
				]
			: [];
	})();

	return (
		<>
			<div className="fade-in-heading animation-delay-2 relative z-10 flex flex-row flex-wrap justify-center gap-3">
				{currentPlatform &&
					(() => {
						const { links } = currentPlatform;

						return (
							// <HomeCTA
							// 	href={
							// 		links?.length === 1
							// 			? `${BASE_DL_LINK}/${currentPlatform.os}/${links[0].arch}`
							// 			: undefined
							// 	}
							// 	className={`z-5 relative !bg-[#88D7FF]`}
							// 	icon={<ArrowCircleDown />}
							// 	text={`Download for ${currentPlatform.name}`}
							// 	onClick={() => {
							// 		plausible('download', {
							// 			props: { os: currentPlatform.name }
							// 		});
							// 		setSelectedPlatform(currentPlatform);
							// 	}}
							// />
							<CtaPrimaryButton platform={currentPlatform} />
						);
					})()}

				<CtaSecondaryButton />
			</div>

			{/* {selectedPlatform?.links && selectedPlatform.links.length > 1 && (
				<div className="z-50 flex flex-row gap-3 mt-4 mb-2 fade-in">
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
			)} */}
			<p className="animation-delay-3 z-30 mt-3 px-6 text-center text-sm text-gray-400 fade-in">
				{latestVersion}
				{formattedVersion && (
					<>
						<span className="mx-2 opacity-50">|</span>
						{formattedVersion}
					</>
				)}
				{note && (
					<>
						<span className="mx-2 opacity-50">|</span>
						{note}
					</>
				)}
			</p>
			{/* Platform icons */}
			{/* <div className="relative z-10 flex gap-3 mt-5">
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
			</div> */}
			{/* Docker Dialog */}
			{/* <DockerDialog open={dockerDialogOpen} setOpen={setDockerDialogOpen} /> */}
		</>
	);
}
