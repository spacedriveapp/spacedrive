import { Github as GithubLogo } from '@sd/assets/svgs/brands';
import React from 'react';

export const Github = () => {
	return (
		<div className="flex w-full flex-col p-4">
			<h1 className="pt-[16px] text-xl">Free and open-source. See for yourself.</h1>
			<h2 className="pb-[36px] pt-[16px] text-lg text-ink-faint">
				When we promise strong privacy and encryption, we mean it. Our app’s source code is
				entirely open-source and available on GitHub, so if you’re wondering what Spacedrive
				does with your data or have an improvement to share, you’re welcome to do so — we
				welcome and appreciate contributions!
			</h2>
			<a href="https://github.com/spacedriveapp/spacedrive" target="_blank">
				<button className="inline-flex items-center justify-center gap-[10px] rounded-xl bg-slate-800 px-[16px] py-[10px]">
					<GithubLogo />
					<span>View source on GitHub</span>
				</button>
			</a>
		</div>
	);
};
