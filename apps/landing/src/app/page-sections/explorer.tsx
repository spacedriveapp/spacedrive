import React from 'react';

import { BentoBox } from '../bento-box';

export const Explorer = () => {
	return (
		<div className="mx-auto flex w-full max-w-[1200px] flex-col flex-wrap items-center gap-10 p-4">
			<h1 className="flex-1 self-start text-2xl font-semibold leading-8 md:text-3xl md:leading-10">
				Explorer.{' '}
				<span className="bg-gradient-to-r from-zinc-400 to-zinc-600 bg-clip-text text-transparent">
					Browse and manage <br />
					your data like never before.
				</span>
			</h1>
			<div className="flex flex-col gap-5 lg:flex-row">
				<BentoBox
					className="bento-card-border-left"
					imageSrc="/images/new/bento_1.svg"
					imageAlt="Library"
					title="Seamless Sync & Access"
					titleColor="#63C3F3"
					description="Whether online or offline, instantly access your data anytime, anywhere. Keeping everything updated and available across your devices."
				/>
				<BentoBox
					imageSrc="/images/new/bento_2.svg"
					imageAlt="Lock"
					title="Privacy & Control"
					titleColor="#6368F3"
					description="Your data is yours. With Spacedrive’s top-notch security, only you can access your information — no third parties, no exceptions."
				/>
				<BentoBox
					className="bento-card-border-right"
					imageSrc="/images/new/bento_3.svg"
					imageAlt="Tags"
					title="Effortless Organization"
					titleColor="#DF63F3"
					description="Keep your digital life organized with automatic categorization and smart structuring, making it easy to find what you need instantly."
				/>
			</div>
		</div>
	);
};
