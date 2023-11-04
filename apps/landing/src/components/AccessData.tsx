import Image from 'next/image';
import React from 'react';

const AccessData = () => {
	return (
		<div className="my-[150px] md:my-[300px]">
			<Image
				width={390}
				height={300}
				quality={100}
				className="mx-auto mb-10"
				alt="data globe"
				src="/images/misc/globe.webp"
			/>
			<div className="relative">
				<h1
					className="bg-gradient-to-r from-white to-violet-400 bg-clip-text text-center
						 text-[25px] font-bold text-transparent md:text-[30px]"
				>
					Access data from anywhere
				</h1>
				<p className="mx-auto w-full max-w-[800px] text-center text-sm text-ink-faint md:text-lg">
					users can enjoy the freedom of accessing their important files, documents, and
					media assets from any device with an internet connection, ensuring productivity
					and convenience on the go.
				</p>
			</div>
		</div>
	);
};

export default AccessData;
