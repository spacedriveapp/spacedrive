import Image from 'next/image';
import React from 'react';

const CloudStorage = () => {
	return (
		<div className="relative mt-[200px] w-full max-w-[960px] md:mt-[250px]">
			<div className="absolute-horizontal-center top-[-100px] h-[248px] w-[500px] md:top-[-195px] md:w-[960px]">
				<Image
					width={960}
					height={248}
					quality={100}
					className="mb-10"
					alt="art"
					src="/images/cloudstorage.webp"
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
				<div className="flex flex-col justify-center w-full gap-5 md:flex-row">
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
