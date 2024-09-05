import { Github as GithubLogo } from '@sd/assets/svgs/brands';
import React from 'react';
import { CtaSecondaryButton } from '~/components/cta-secondary-button';

export const Github = () => {
	return (
		<section className="container flex flex-col flex-wrap items-start w-full px-4 mx-auto">
			<hgroup>
				<h1 className="self-start flex-1 text-2xl font-bold leading-8 md:text-3xl md:leading-10">
					Free and open-source.{' '}
					<span className="font-semibold text-transparent bg-gradient-to-r from-zinc-400 to-zinc-500/70 bg-clip-text">
						See for yourself.
					</span>
				</h1>
				<p className="mt-[12px] font-plex text-lg leading-[125%] tracking-[0.01em] text-ink-dull">
					When we promise strong privacy and encryption, we mean it. Our app’s source code
					is entirely open-source and available on GitHub, so if you’re wondering what
					Spacedrive does with your data or have an improvement to share, you’re welcome
					to do so — we welcome and appreciate contributions!
				</p>
			</hgroup>
			<div className="relative z-10 mt-[36px]">
				<CtaSecondaryButton
					icon={<GithubLogo fill="#CBDBEC" className="size-5 opacity-60" />}
					href="https://github.com/spacedriveapp/spacedrive"
					target="_blank"
				>
					View source on GitHub
				</CtaSecondaryButton>
			</div>
		</section>
	);
};
