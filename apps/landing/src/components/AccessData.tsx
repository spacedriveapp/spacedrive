import Image from 'next/image';
import React from 'react';

const AccessData = () => {
	return (
		<div className="mb-[150px] md:mb-[250px]">
			<h3
				className="bg-gradient-to-r from-white to-violet-400 bg-clip-text text-center
						 text-[25px] font-bold text-transparent md:text-[30px]"
			>
				Access data from anywhere
			</h3>
			<p className="mx-auto w-full max-w-[800px] text-center text-sm text-ink-faint md:text-lg">
				users can enjoy the freedom of accessing their important files, documents, and media
				assets from any device with an internet connection, ensuring productivity and
				convenience on the go.
			</p>
			<div className="relative h-auto">
				<Image
					width={950}
					height={300}
					quality={100}
					className="mx-auto mb-10"
					alt="globe"
					src="/images/globe.webp"
				/>
				<div
					className="absolute-center left-0 top-0 h-[80px] w-[200px] bg-[#BC93FF]
							 opacity-40 blur-[50px] md:h-[150px] md:w-[450px] md:blur-[120px]"
				/>
			</div>
		</div>
	);
};

export default AccessData;
