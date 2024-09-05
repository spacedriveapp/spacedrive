import clsx from 'clsx';
import Image, { StaticImageData } from 'next/image';
import spacedropIllustration from '~/assets/illustration/spacedrop.webp';

export const Features = () => {
	return (
		<section className="container relative mx-auto flex flex-row flex-wrap p-4">
			<h2 className="sr-only">Features</h2>
			{/** Lines & middle circle */}
			<div className="absolute inset-x-0 mx-auto h-full w-px bg-gradient-to-b from-transparent via-[#6C708F] to-transparent" />
			<div className="absolute flex h-px w-full self-center bg-gradient-to-r from-transparent via-[#6C708F] to-transparent" />
			<div className="absolute left-1/2 top-1/2 z-10 mx-auto size-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-[#636783]" />
			{/** Features */}
			{info.map((item, index) => (
				<Feature
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
	image?: { src?: StaticImageData; alt?: string };
	className?: string;
	titleClassName?: string;
}

const Feature = ({
	title,
	description,
	className,
	titleClassName,
	image: { src: image, alt = '' } = {}
}: Props) => {
	return (
		<li className={clsx('flex h-[700px] flex-[50%] flex-col gap-3 pl-16 pt-16', className)}>
			<h3 className={clsx('text-2xl font-semibold', titleClassName)}>{title}</h3>
			<p className="w-full max-w-[390px] text-ink-faint">{description}</p>
			{image && (
				<Image
					className="mt-8 px-8"
					loading="eager"
					layout="responsive"
					src={image}
					quality={100}
					alt={alt}
				/>
			)}
		</li>
	);
};

const info = [
	{
		title: 'Spacedrop',
		description:
			'Quickly send files between devices. Just select what you want to share and instantly transfer it to nearby devices on the same network.',
		image: {
			src: spacedropIllustration,
			// TODO: write alt text for spacedrop image
			alt: ''
		}
	},
	{
		title: 'Tags',
		description:
			'Organize and find your files faster by assigning custom tags to your folders and documents. Simplify your data management with easy categorization.'
	},
	{
		title: 'End-To-End Encryption',
		description:
			'Any time you send files across a network with Spacedrive, it’s end-to-end encrypted — ensuring that only you can access your files. Ever.'
	},
	{
		title: 'Extensions',
		description:
			'Install add-ons to customize Spacedrive with extra features and integrations, tailoring it to your unique workflow.'
	}
] satisfies {
	title: string;
	description: string;
	image?: { src: StaticImageData; alt?: string };
}[];
