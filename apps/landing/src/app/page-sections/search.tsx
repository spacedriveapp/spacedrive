export const Search = () => {
	return (
		<section className="container mx-auto flex w-full flex-col flex-wrap items-start overflow-hidden">
			<h2 className="w-full max-w-[600px] flex-1 self-start p-4 text-2xl font-semibold leading-7 md:text-3xl md:leading-10">
				Search.{' '}
				<span className="bg-gradient-to-r from-zinc-400 to-zinc-600 bg-clip-text text-transparent">
					Find what you’re looking for with ease using advanced filters.
				</span>
			</h2>
			<div className="mt-4 w-full p-4">
				<video
					className="h-auto w-full rounded-xl object-fill"
					autoPlay
					playsInline
					muted
					controls={false}
					loop
					src="/videos/Spacedrive_search.webm"
				/>
			</div>
		</section>
	);
};
