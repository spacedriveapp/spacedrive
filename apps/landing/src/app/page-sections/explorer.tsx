import React from 'react';
import { twMerge } from 'tailwind-merge';
import libraryArt from '~/assets/bento/library.svg?url';
import lockArt from '~/assets/bento/lock.svg?url';
import tagsArt from '~/assets/bento/tags.svg?url';
import { BentoBox } from '~/components/bento-box';

export const Explorer = () => {
	return (
		<div className="container mx-auto flex flex-col flex-wrap items-center gap-10 p-4">
			<h1 className="flex-1 self-start text-2xl font-semibold leading-8 md:text-3xl md:leading-10 lg:self-start">
				Explorer.{' '}
				<span className="bg-gradient-to-r from-zinc-400 to-zinc-600 bg-clip-text text-transparent">
					{/* Some controlled line breaks here based on breakpoint to make sure the breaks looks nice always :) */}
					<br className="lg:hidden" />
					Browse and manage your data
					<br className="sm:hidden" /> like never before.
				</span>
			</h1>
			<div
				className={twMerge(
					'grid w-full grid-cols-3 grid-rows-2 gap-5 max-lg:grid-cols-1 lg:grid-rows-1'
				)}
			>
				<BentoBox
					className="bento-border-top lg:bento-border-left"
					imageSrc={libraryArt}
					imageAlt="Library"
					title="Seamless Sync & Access"
					titleColor="#63C3F3"
					description="Whether online or offline, instantly access your data anytime, anywhere. Keeping everything updated and available across your devices."
				/>
				<BentoBox
					imageSrc={lockArt}
					imageAlt="Lock"
					title="Privacy & Control"
					titleColor="#6368F3"
					description="Your data is yours. With Spacedrive’s top-notch security, only you can access your information — no third parties, no exceptions."
				/>
				<BentoBox
					className="bento-border-bottom lg:bento-border-right"
					imageSrc={tagsArt}
					imageAlt="Tags"
					title="Effortless Organization"
					titleColor="#DF63F3"
					description="Keep your digital life organized with automatic categorization and smart structuring, making it easy to find what you need instantly."
				/>
			</div>
		</div>
	);
};
