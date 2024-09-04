import clsx from 'clsx';
import Image from 'next/image';
import React from 'react';

export const Features = () => {
	return (
		<div className="relative mx-auto flex h-auto w-full max-w-[1200px] flex-col flex-wrap gap-14 p-4 md:flex-row md:gap-0">
			{/** Lines & middle circle */}
			<div className="absolute inset-x-0 mx-auto hidden h-[90%] w-px bg-gradient-to-b from-transparent via-[#6C708F]/30 to-transparent md:flex" />
			<div className="absolute hidden h-px w-full self-center bg-gradient-to-r from-transparent via-[#6C708F]/30 to-transparent md:flex" />
			{/** Features */}
			{info.map((item, index) => (
				<Feature
					{...item}
					key={index}
					titleClassName={clsx((index === 1 || index === 3) && 'self-start')}
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
	size?: { width: number; height?: number };
	scale?: number;
}

const Feature = ({ title, description, className, titleClassName, imageSrc, size }: Props) => {
	const imageSize = size ?? { width: 500, height: 500 };
	return (
		<div className={clsx('flex h-[580px] flex-[50%] flex-col gap-3 pt-16 md:pl-16', className)}>
			<div className="flex flex-col gap-1">
				<h1 className={clsx('text-2xl font-semibold', titleClassName)}>{title}</h1>
				<p className="w-full max-w-[390px] text-ink-faint">{description}</p>
			</div>
			{/* Container needed to force <Image> into custom sizes */}
			<div className="w-auto h-auto mx-auto">
				<Image
					className="px-8 mt-8"
					loading="eager"
					layout="responsive"
					width={imageSize.width}
					height={imageSize.height}
					quality={100}
					alt={title}
					src={imageSrc}
				/>
			</div>
		</div>
	);
};

const info: {
	title: string;
	description: string;
	imageSrc: string;
	scale?: number;
}[] = [
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
		imageSrc: '/images/bento/tags.webp'
	},
	{
		title: 'End-To-End Encryption',
		description:
			'Any time you send files across a network with Spacedrive, it’s end-to-end encrypted — ensuring that only you can access your files. Ever.',
		imageSrc: '/images/bento/vault.webp'
	},
	{
		title: 'Extensions',
		description:
			'Install add-ons to customize Spacedrive with extra features and integrations, tailoring it to your unique workflow.',
		imageSrc: '/images/bento/extensions.webp'
	}
];
