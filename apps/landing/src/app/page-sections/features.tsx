import clsx from 'clsx';
import Image, { StaticImageData } from 'next/image';
import extensionsIllustration from '~/assets/illustration/extensions.webp';
import spacedropIllustration from '~/assets/illustration/spacedrop.webp';
import tagsIllustration from '~/assets/illustration/tags.webp';
import vaultIllustration from '~/assets/illustration/vault.webp';

export const Features = () => {
	return (
		<section className="container relative mx-auto flex items-center justify-center p-4">
			<h2 className="sr-only">Features</h2>
			{/** Lines & middle circle */}
			<div className="absolute inset-x-0 mx-auto hidden h-[90%] w-px bg-gradient-to-b from-transparent via-[#6C708F]/30 to-transparent lg:flex" />
			<div className="absolute hidden h-px w-full self-center bg-gradient-to-r from-transparent via-[#6C708F]/30 to-transparent lg:flex" />
			<div className="absolute left-1/2 top-1/2 z-10 mx-auto hidden size-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-[#636783] lg:flex" />
			{/** Features */}
			<div className="grid grid-cols-1 max-lg:gap-14 lg:grid-cols-2">
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
			</div>
		</section>
	);
};

interface FeatureImage {
	src: StaticImageData;
	alt: string;
	maxWidth?: number;
	maxHeight?: number;
}

interface Props {
	title: string;
	description: string;
	image: FeatureImage;
	className?: string;
	titleClassName?: string;
	scale?: number;
}

const Feature = ({
	title,
	description,
	className,
	titleClassName,
	image: { src: image, alt = '', maxWidth, maxHeight }
}: Props) => {
	return (
		<div className={clsx('flex w-full flex-col gap-3 px-8 pb-8 pt-6', className)}>
			<div className="flex flex-col gap-1">
				<h1 className={clsx('text-2xl font-semibold', titleClassName)}>{title}</h1>
				<p className="w-full max-w-96 text-ink-faint">{description}</p>
			</div>
			<Image
				className="mx-auto my-6 size-full overflow-hidden object-contain"
				loading="eager"
				quality={100}
				style={{
					maxWidth,
					maxHeight
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
			alt: '',
			maxWidth: 450
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
			maxWidth: 260
		}
	},
	{
		title: 'End-To-End Encryption',
		description:
			'Any time you send files across a network with Spacedrive, it’s end-to-end encrypted — ensuring that only you can access your files. Ever.',
		image: {
			src: vaultIllustration,
			// TODO: write alt text
			alt: '',
			maxWidth: 450
		}
	},
	{
		title: 'Extensions',
		description:
			'Install add-ons to customize Spacedrive with extra features and integrations, tailoring it to your unique workflow.',
		image: {
			src: extensionsIllustration,
			// TODO: write alt text
			alt: '',
			maxWidth: 280
		}
	}
] satisfies {
	title: string;
	description: string;
	image: FeatureImage;
}[];
