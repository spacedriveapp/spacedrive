import Image from 'next/image';
import React from 'react';

export const Search = () => {
	return (
		<div className="mx-auto flex w-full max-w-[1200px] flex-col flex-wrap items-start p-4">
			<h1 className="flex-1 self-start text-2xl font-semibold leading-8 md:text-3xl md:leading-10">
				Search.{' '}
				<span className="bg-gradient-to-r from-zinc-400 to-zinc-600 bg-clip-text text-transparent">
					Find what you’re looking for <br></br> with ease using advanced filters.
				</span>
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
