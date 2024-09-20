'use client';

import { GooglePlayLogo } from '@phosphor-icons/react';
import { Apple } from '@sd/assets/svgs/brands';
import Image from 'next/image';
import { tw } from '@sd/ui';

const DownloadButton = tw.button`flex w-fit flex-row items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-900 p-2.5 text-[13px] transition-all duration-300 hover:border-zinc-700 hover:bg-zinc-800`;

const Mobile = () => {
	return (
		<div className="container mx-auto mt-10 flex flex-col flex-wrap items-center gap-10 px-4">
			<Image
				style={{
					maxWidth: 900,
					maxHeight: 600
				}}
				loading="eager"
				quality={100}
				width={900}
				height={600}
				className="object-contain"
				layout="responsive"
				src="/images/mobile.webp"
				alt="Mobile"
			/>
			<div className="flex flex-col gap-1">
				<h1 className="flex flex-col self-center text-center text-2xl font-semibold md:flex-row md:text-3xl">
					Cross platform.&nbsp;
					<span className="bg-gradient-to-r from-zinc-400 to-zinc-600 bg-clip-text text-transparent">
						<br className="hidden lg:visible" />
						Available on iOS and Android
					</span>
				</h1>
				<p className="w-full max-w-[600px] text-center text-ink-faint">
					Using the mobile app, you can sync your files across all your devices. Take your
					personal data with you wherever you are!
				</p>
				<div className="mx-auto mt-4 flex flex-row flex-wrap justify-center gap-4">
					<DownloadButton>
						<GooglePlayLogo />
						Open Play Store
					</DownloadButton>
					<DownloadButton>
						<Apple className="size-4" />
						Open App Store
					</DownloadButton>
				</div>
			</div>
		</div>
	);
};

export default Mobile;
