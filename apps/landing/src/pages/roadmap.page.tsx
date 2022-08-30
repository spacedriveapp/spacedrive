import { Button } from '@sd/ui';
import React from 'react';
import { Helmet } from 'react-helmet';

import { Folder } from '../../../../packages/interface/src/components/icons/Folder';

function Page() {
	const items = [
		{
			when: 'Complete',
			completed: true,
			title: 'File Discovery',
			description:
				'Scan devices, drives and cloud accounts to build a directory of all files with metadata.'
		},
		{
			title: 'Preview generation',
			completed: true,
			description: 'Auto generate lower resolution stand-ins for image and video.'
		},
		{
			title: 'Statistics',
			completed: true,
			description: 'Total capacity, index size, preview media size, free space etc.'
		},
		{
			when: 'In Progress',
			title: 'File Explorer',
			description:
				' Browse online/offline storage locations, view files with metadata, perform basic CRUD.'
		},
		{
			title: 'Realtime synchronization',
			description:
				'Data index synchronized in realtime between devices, prioritizing peer-to-peer LAN connections (WiFi sync).'
		},
		{
			title: 'Self hosted',
			description:
				'Spacedrive can be deployed as a service, behaving as just another device powering your personal cloud.'
		},
		{
			when: 'MVP',
			title: 'Photos',
			description: 'Photos and video albums similar to Apple/Google photos.'
		},
		{
			title: 'Search',
			description: 'Deep search into your filesystem with a keybind, including offline locations.'
		},
		{
			title: 'Tags',
			description:
				'Define routines on custom tags to automate workflows, easily tag files individually, in bulk and automatically via rules.'
		},
		{
			title: 'Extensions',
			description:
				'Build tools on top of Spacedrive, extend functionality and integrate third party services. Extension directory on spacedrive.com/extensions.'
		},
		{
			when: 'Post-MVP',
			title: 'Spacedrive Cloud',
			description:
				"We'll host an always-on cloud device for you, with pay-as-you-go plans for storage."
		},
		{
			title: 'Cloud integration',
			description:
				'Index & backup to Apple Photos, Google Drive, Dropbox, OneDrive & Mega + easy API for the community to add more.'
		},
		{
			title: 'Encrypted vault(s)',
			description:
				'Effortlessly manage & encrypt sensitive files, built on top of VeraCrypt. Encrypt individual files or create flexible-size vaults.'
		},
		{
			title: 'Key manager',
			description:
				'View, mount, dismount and hide keys. Mounted keys automatically unlock respective areas of your filesystem.'
		},
		{
			title: 'Redundancy Goal',
			description:
				'Ensure a specific amount of copies exist for your important data, discover at-risk files and monitor device/drive health.'
		},
		{
			title: 'Timeline',
			description:
				'View a linear timeline of content, travel to any time and see media represented visually.'
		},
		{
			title: 'Media encoder',
			description:
				'Encode video and audio into various formats, use Tags to automate. Built with FFMPEG.'
		},
		{
			title: 'Workers',
			description:
				'Utilize the compute power of your devices in unison to encode and perform tasks at increased speeds.'
		}
	];

	return (
		<>
			<Helmet>
				<title>Roadmap - Spacedrive</title>
				<meta name="description" content="What can Spacedrive do?" />
			</Helmet>
			<div className="container flex flex-col gap-20 max-w-4xl p-4 m-auto mt-32 mb-20 prose lg:prose-xs dark:prose-invert">
				<section className="flex flex-col items-center">
					<Folder className="w-24 pointer-events-none" />
					<h1 className="text-5xl leading-snug fade-in-heading mb-0 text-center">
						What's next for Spacedrive?
					</h1>
					<p className="text-gray-400 animation-delay-2 fade-in-heading text-center">
						Here is a list of the features we are working on, and the progress we have made so far.
					</p>
				</section>
				<section className="grid grid-cols-[auto_1fr] grid-flow-row auto-cols-auto gap-x-4">
					{items.map((item, i) => (
						<>
							{/* Using span so i can use the group-last-of-type selector */}
							<span className="max-w-[10rem] flex items-start first:items-start justify-end gap-4 group">
								<h3
									className={
										`m-0 hidden lg:block text-right ` +
										(i === 0 ? '-translate-y-1/4' : '-translate-y-1/2')
									}
								>
									{item.when}
								</h3>
								<div className="w-2 h-full group-first:rounded-t-full group-last-of-type:rounded-b-full flex lg:items-center group-first:mt-2">
									<div
										className={
											'w-full h-full flex ' +
											(item.completed ? 'bg-primary-500 z-10' : 'bg-gray-550')
										}
									>
										{item?.when ? (
											<div className="h-4 w-4 border-2 border-gray-200 bg-primary-500 rounded-full mt-5 group-first:mt-0 lg:mt-0 group-first:self-start absolute -translate-x-1/4 -translate-y-1/2 z-20">
												&zwj;
											</div>
										) : (
											<div className="z-20">&zwj;</div>
										)}
									</div>
								</div>
							</span>
							<div className="flex flex-col items-start justify-center gap-4 group">
								{item?.when && (
									<h3 className="mb-0 group-first-of-type:m-0 lg:hidden">{item.when}</h3>
								)}
								<div className="border border-gray-500 rounded-xl p-4 flex flex-col space-y-2 my-2 group-first-of-type:mt-0 group-last:mb-0 w-full">
									<h3 className="my-1">{item.title}</h3>
									<p>{item.description}</p>
								</div>
							</div>
						</>
					))}
				</section>
				<section className="bg-gray-850 rounded-xl p-8 space-y-2">
					<h2 className="my-1">That's not all.</h2>
					<p>
						We're always open to ideas and feedback over{' '}
						<a href="https://github.com/spacedriveapp/spacedrive/discussions">here</a> and we have a{' '}
						<a href="/blog">blog</a> where you can find the latest news and updates.
					</p>
				</section>
			</div>
		</>
	);
}

export default Page;
