import clsx from 'clsx';
import Image, { StaticImageData } from 'next/image';
import extenstionsIllustration from '~/assets/illustration/extensions.webp';
import spacedropIllustration from '~/assets/illustration/spacedrop.webp';
import tagsIllustration from '~/assets/illustration/tags.webp';
import vaultIllustration from '~/assets/illustration/vault.webp';

export const Features = () => {
	return (
		<section className="container relative mx-auto flex h-auto flex-col flex-wrap items-center justify-center gap-14 p-4 md:flex-row md:gap-0">
			<h2 className="sr-only">Features</h2>
			{/** Lines & middle circle */}
			<div className="absolute inset-x-0 mx-auto hidden h-[90%] w-px bg-gradient-to-b from-transparent via-[#6C708F]/30 to-transparent md:flex" />
			<div className="absolute hidden h-px w-full self-center bg-gradient-to-r from-transparent via-[#6C708F]/30 to-transparent md:flex" />
			<div className="absolute left-1/2 top-1/2 z-10 mx-auto size-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-[#636783]" />
			{/** Features */}
			{info.map((item, index) => (
				<Feature
					{...item}
					key={index}
					titleClassName={clsx((index === 1 || index === 3) && 'self-start')}
					title={item.title}
					image={item.image}
					description={item.description}
				/>
			))}
		</section>
	);
};

interface Props {
	title: string;
	description: string;
	image: { src: StaticImageData; alt: string; maxWidth?: number };
	className?: string;
	titleClassName?: string;
	scale?: number;
}

const Feature = ({
	title,
	description,
	className,
	titleClassName,
	image: { src: image, alt = '', maxWidth }
}: Props) => {
	return (
		<div className={clsx('flex h-[580px] flex-[50%] flex-col gap-3 pt-16 md:pl-16', className)}>
			<div className="flex flex-col gap-1">
				<h1 className={clsx('text-2xl font-semibold', titleClassName)}>{title}</h1>
				<p className="w-full max-w-[390px] text-ink-faint">{description}</p>
			</div>
			{/* Container needed to force <Image> into custom sizes */}
			<Image
				className="mt-8 h-full w-full object-contain px-8"
				loading="eager"
				layout="responsive"
				quality={100}
				style={{
					maxWidth
				}}
				src={image}
				alt={alt}
			/>
		</div>
	);
};

const info = [
	{
		title: 'Spacedrop',
		description:
			'Quickly send files between devices. Just select what you want to share and instantly transfer it to nearby devices on the same network.',
		image: {
			src: spacedropIllustration,
			// TODO: write alt text
			alt: ''
		}
	},
	{
		title: 'Tags',
		description:
			'Organize and find your files faster by assigning custom tags to your folders and documents. Simplify your data management with easy categorization.',
		image: {
			src: tagsIllustration,
			// TODO: write alt text
			alt: '',
			maxWidth: 320
		}
	},
	{
		title: 'End-To-End Encryption',
		description:
			'Any time you send files across a network with Spacedrive, it’s end-to-end encrypted — ensuring that only you can access your files. Ever.',
		image: {
			src: vaultIllustration,
			// TODO: write alt text
			alt: ''
		}
	},
	{
		title: 'Extensions',
		description:
			'Install add-ons to customize Spacedrive with extra features and integrations, tailoring it to your unique workflow.',
		image: {
			src: extenstionsIllustration,
			// TODO: write alt text
			alt: ''
		}
	}
] satisfies {
	title: string;
	description: string;
	image: { src: StaticImageData; alt: string; maxWidth?: number };
}[];
