import clsx from 'clsx';
import Image from 'next/image';
import React from 'react';

export const Features = () => {
	return (
		<div className="relative mx-auto flex w-full max-w-[1200px] flex-row flex-wrap p-4">
			{/** Lines & middle circle */}
			<div className="absolute inset-x-0 mx-auto h-full w-px bg-gradient-to-b from-transparent via-[#6C708F] to-transparent" />
			<div className="absolute flex h-px w-full self-center bg-gradient-to-r from-transparent via-[#6C708F] to-transparent" />
			<div className="absolute left-1/2 top-1/2 z-10 mx-auto size-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-[#636783]" />
			{/** Features */}
			{info.map((item, index) => (
				<Feature
					key={index}
					className={clsx((index === 1 || index === 3) && 'items-center')}
					titleClassName={clsx((index === 1 || index === 3) && 'self-start pl-24')}
					title={item.title}
					imageSrc={item.imageSrc}
					description={item.description}
				/>
			))}
		</div>
	);
};

interface Props {
	title: string;
	description: string;
	imageSrc: string;
	className?: string;
	titleClassName?: string;
}

const Feature = ({ title, description, className, titleClassName, imageSrc }: Props) => {
	return (
		<div className={clsx('flex h-[700px] flex-[50%] flex-col gap-3 pt-16', className)}>
			<h1 className={clsx('text-2xl font-semibold', titleClassName)}>{title}</h1>
			<p className="w-full max-w-[390px] text-base text-ink-faint">{description}</p>
			<Image
				className="mt-8 px-8"
				loading="eager"
				layout="responsive"
				width={500}
				quality={100}
				height={500}
				alt={title}
				src={imageSrc}
			/>
		</div>
	);
};

const info = [
	{
		title: 'Spacedrop',
		description:
			'Quickly send files between devices. Just select what you want to share and instantly transfer it to nearby devices on the same network.',
		imageSrc: '/images/bento/spacedrop.webp'
	},
	{
		title: 'Tags',
		description:
			'Organize and find your files faster by assigning custom tags to your folders and documents. Simplify your data management with easy categorization.',
		imageSrc: ''
	},
	{
		title: 'End-To-End Encryption',
		description:
			'Any time you send files across a network with Spacedrive, it’s end-to-end encrypted — ensuring that only you can access your files. Ever.',
		imageSrc: ''
	},
	{
		title: 'Extensions',
		description:
			'Install add-ons to customize Spacedrive with extra features and integrations, tailoring it to your unique workflow.',
		imageSrc: ''
	}
];
