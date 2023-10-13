import clsx from 'clsx';
import { useInView } from 'framer-motion';
import Image from 'next/image';
import React, { useRef } from 'react';

import CloudStorageArt from './CloudStorageArt';

const CloudStorage = () => {
	const ref = useRef<HTMLDivElement>(null);
	const isInView = useInView(ref, {
		amount: 0.5,
		once: true
	});

	return (
		<div
			ref={ref}
			className={clsx(
				'relative mt-[200px] w-full max-w-[960px] opacity-0  md:mt-[250px]',
				isInView && 'fade-in-heading'
			)}
		>
			<div className="absolute-horizontal-center top-[-100px] h-[248px] w-[500px] md:top-[-55px] md:w-[960px]">
				<div className="relative right-[270px] z-10 md:right-0">
					<CloudStorageArt />
				</div>
				<Image
					src="/images/cloudgradient.webp"
					className="absolute inset-x-0 top-[-100px] mx-auto"
					width={560}
					height={200}
					alt="cloud gradient"
				/>
			</div>
			<div className="mt-[60px] flex w-full flex-col flex-wrap items-center justify-center gap-5 md:flex-row">
				<CloudCard title="Dropbox" logoUrl="/images/dropbox.svg" imageWidth={49} />
				<CloudCard title="iCloud" logoUrl="/images/icloud.svg" />
				<CloudCard
					title="Google drive"
					logoUrl="/images/google-drive.svg"
					imageWidth={53}
				/>
				<div className="flex w-full flex-col justify-center gap-5 md:flex-row">
					<CloudCard imageWidth={45} title="Mega" logoUrl="/images/mega.svg" />
					<CloudCard title="Amazon S3" logoUrl="/images/s3.svg" imageWidth={40} />
				</div>
			</div>
			<h1
				className="mt-[50px] bg-gradient-to-r from-white to-blue-400 bg-clip-text text-center text-[30px] font-bold
						 leading-10 text-transparent"
			>
				Coming soon
			</h1>
			<h1
				className="bg-gradient-to-r from-white to-blue-300 bg-clip-text
						  text-center text-[20px] text-transparent md:text-[40px] md:leading-[50px]"
			>
				Combine all storage locations & clouds
			</h1>
		</div>
	);
};

interface Props {
	logoUrl: string;
	title: string;
	imageWidth?: number;
	children?: React.ReactNode;
}

const CloudCard = ({ logoUrl, title, imageWidth = 70, children }: Props) => {
	return (
		<div
			className="flex w-full flex-col justify-center rounded-md border border-[#161524]
			 bg-[#080710]/30 py-6 text-center backdrop-blur-sm transition-all duration-200 hover:brightness-125 md:h-[165px] md:basis-[30%]"
		>
			{children}
			<div className="relative z-10">
				<Image
					width={imageWidth}
					height={100}
					quality={100}
					alt="cloud storage"
					className="mx-auto"
					src={logoUrl}
				/>
				<p className="mt-3 font-semibold">{title}</p>
			</div>
		</div>
	);
};

export default CloudStorage;
