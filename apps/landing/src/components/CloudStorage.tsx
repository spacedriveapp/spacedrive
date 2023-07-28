import Image from 'next/image';
import React from 'react';

const CloudStorage = () => {
	return (
		<div className="mt-[200px] md:mt-[250px]">
			<div className="relative h-auto">
				<Image
					width={590}
					height={300}
					quality={100}
					className="mx-auto mb-10"
					alt="cloud storage"
					src="/images/clouds.webp"
				/>
				<div
					className="absolute-horizontal-center left-0 top-0 h-[150px]
							 w-[450px] bg-violet-500 opacity-30 blur-[120px]"
				/>
			</div>
			<h3
				className="bg-gradient-to-r from-white to-violet-400 bg-clip-text text-center
						 text-[30px] font-bold text-transparent"
			>
				Coming soon
			</h3>
			<h1
				className="bg-gradient-to-r from-white to-violet-300 bg-clip-text
						  text-center text-[20px] text-transparent md:text-[40px]"
			>
				Combine all storage locations & clouds
			</h1>
			<div className="mt-[60px] flex-col items-center md:flex md:flex-row md:justify-between">
				<CloudCard title="Outlook" logoUrl="/images/outlook.svg">
					<div
						className="absolute left-0 top-0 z-10 h-full w-[1px]
								bg-gradient-to-b from-indigo-300 to-transparent opacity-50 md:left-auto md:right-0"
					/>
				</CloudCard>
				<CloudCard title="iCloud" logoUrl="/images/icloud.svg">
					<div
						className="absolute left-0 top-0 z-10 h-full w-[1px] bg-gradient-to-b from-indigo-300
								 to-transparent opacity-50 md:hidden"
					/>
				</CloudCard>
				<CloudCard title="Google drive" logoUrl="/images/google-drive.svg" imageWidth={55}>
					<div
						className="absolute left-0 top-0 z-10 h-full w-[1px] bg-gradient-to-b
								 from-indigo-300 to-transparent opacity-50"
					/>
				</CloudCard>
			</div>
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
		<div className="relative flex-1 md:flex-[33%]">
			<div className="cloud-card-gradient-bg cloud-card-hover bg-transparent py-6 text-center ">
				{children}
				<div className="relative z-10">
					<Image
						width={imageWidth}
						height={100}
						quality={100}
						alt="icloud"
						className="mx-auto"
						src={logoUrl}
					/>
					<p className="mt-3 font-semibold">{title}</p>
				</div>
			</div>
		</div>
	);
};

export default CloudStorage;
