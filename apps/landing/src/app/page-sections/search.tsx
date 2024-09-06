import React from 'react';

export const Search = () => {
	return (
		<section className="container flex flex-col flex-wrap items-start w-full mx-auto overflow-hidden">
			<h2 className="self-start flex-1 p-4 text-2xl font-semibold leading-8 md:text-3xl md:leading-10">
				Search.{' '}
				<span className="text-transparent bg-gradient-to-r from-zinc-400 to-zinc-600 bg-clip-text">
					Find what you’re looking for <br></br> with ease using advanced filters.
				</span>
			</h2>
			<div className="relative mx-auto h-[740px] w-full max-w-[600px] overflow-hidden md:h-[1150px] md:max-w-[1470px]">
				<div
					style={{
						backgroundSize: 'cover',
						backgroundImage: 'url(/images/search.webp)'
					}}
					className="absolute w-full h-full ml-3 bottom-36 md:ml-0"
				/>
			</div>
		</section>
	);
};
