import { Github as GithubLogo } from '@sd/assets/svgs/brands';
import React from 'react';

import { CtaSecondaryButton } from '../cta-secondary-button';

export const Github = () => {
	return (
		<div className="mx-auto flex w-full max-w-[1200px] flex-col flex-wrap items-start p-4">
			<h1 className="flex-1 self-start text-2xl font-semibold leading-8 md:text-3xl md:leading-10">
				Free and open-source.{' '}
				<span className="to-zinc-[#91919A] bg-gradient-to-r from-[#A3A3AE] to-[#67676B] bg-clip-text text-transparent">
					See for yourself.
				</span>
			</h1>
			<h2 className="mt-[12px] font-plex text-[18px] font-[400] leading-[125%] tracking-[0.18px] text-[#CDCDCD]">
				When we promise strong privacy and encryption, we mean it. Our app’s source code is
				entirely open-source and available on GitHub, so if you’re wondering what Spacedrive
				does with your data or have an improvement to share, you’re welcome to do so — we
				welcome and appreciate contributions!
			</h2>
			<div className="mt-[36px]">
				<CtaSecondaryButton
					icon={<GithubLogo fill="#CBDBEC" className="size-5 opacity-60" />}
					href="https://github.com/spacedriveapp/spacedrive"
					target="_blank"
				>
					View source on GitHub
				</CtaSecondaryButton>
			</div>
		</div>
	);
};
