'use client';

import { Discord } from '@sd/assets/svgs/brands';
import { motion } from 'framer-motion';
import { useEffect, useState } from 'react';

import { useCurrentPlatform, type Platform } from '../utils/current-platform';
import { CtaButton } from './cta-button';
import { DownloadButton } from './download-button';

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
		<div className="animation-delay-2 z-30 flex flex-col items-center fade-in">
			<div className="flex flex-col gap-3 sm:flex-row">
				{currentPlatform &&
					(() => {
						const { links } = currentPlatform;

						return <DownloadButton platform={currentPlatform} />;
					})()}
				<CtaButton
					href="https://discord.gg/gTaF2Z44f5"
					icon={<Discord className="size-5 opacity-60" />}
				>
					Chat on Discord
				</CtaButton>
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
		</div>
	);
}
