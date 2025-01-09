'use client';

import { useEffect, useState } from 'react';
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

	const [downloads, setDownloads] = useState<number | null>(null);

	useEffect(() => {
		fetch('/api/github-stats')
			.then((res) => res.json())
			.then((data) => {
				if (data.downloads) {
					setDownloads(data.downloads);
				}
			})
			.catch(console.error);
	}, []);

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
				{downloads !== null && (
					<>
						<span className="mx-2 opacity-50">|</span>
						{new Intl.NumberFormat().format(downloads)} Downloads
					</>
				)}
			</p>
		</>
	);
}
