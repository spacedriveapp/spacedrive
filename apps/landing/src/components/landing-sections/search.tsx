import Image from 'next/image';
import React from 'react';

export const Search = () => {
	return (
		<div className="flex w-full flex-col items-center justify-center p-4">
			<h1 className="text-xl">
				Search. Find what you’re looking for with ease using advanced filters.
			</h1>
			<Image
				loading="eager"
				className="flex items-center justify-center fade-in"
				width={500}
				height={500}
				alt="l"
				src="/images/bento/search.webp"
			/>
		</div>
	);
};
