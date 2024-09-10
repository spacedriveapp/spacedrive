import React from 'react';

export const Search = () => {
	return (
		<section className="container mx-auto flex w-full flex-col flex-wrap items-start overflow-hidden">
			<h2 className="w-full max-w-[600px] flex-1 self-start p-4 text-2xl font-semibold leading-7 md:text-3xl md:leading-10">
				Search.{' '}
				<span className="bg-gradient-to-r from-zinc-400 to-zinc-600 bg-clip-text text-transparent">
					Find what you’re looking for with ease using advanced filters.
				</span>
			</h2>
			<div className="video-container max-size-[500px] mx-auto mb-20 flex w-full justify-center overflow-visible md:mb-0 md:h-auto md:w-full lg:max-h-[1000px] lg:max-w-[1400px]">
				<div className="video-wrapper ml-10 mt-16 overflow-hidden sm:ml-16 md:ml-40 md:h-[700px] lg:ml-56 lg:mt-0 lg:h-[600px] xl:h-[860px]">
					<div className="absolute left-20 top-16 z-10 size-[300px] rounded-full bg-white opacity-30 mix-blend-overlay blur-[35px]" />
					<div className="absolute left-52 top-24 h-[400px] w-[300px] rounded-full bg-gradient-to-t from-indigo-300 to-fuchsia-300 opacity-40 mix-blend-overlay blur-[25px] md:top-0 md:blur-[45px] lg:top-12" />
					<video
						className="h-auto w-full rounded-[10px] border border-indigo-500/30"
						autoPlay
						playsInline
						muted
						controls={false}
						loop
						src="/videos/search.mp4"
					/>
				</div>
			</div>
		</section>
	);
};
