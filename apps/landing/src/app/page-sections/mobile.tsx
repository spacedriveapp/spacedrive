import Image from 'next/image';

const Mobile = () => {
	return (
		<div className="container mx-auto mt-10 flex flex-col flex-wrap items-center gap-10 px-4">
			<Image
				style={{
					maxWidth: 635,
					maxHeight: 530
				}}
				loading="eager"
				quality={100}
				width={636}
				height={530}
				layout="responsive"
				className="object-contain"
				src="/images/mobile.webp"
				alt="Mobile"
			/>
			<div className="flex flex-col gap-1">
				<h1 className="flex flex-col self-center text-center text-2xl font-semibold md:flex-row md:text-3xl">
					Cross platform.&nbsp;
					<span className="bg-gradient-to-r from-zinc-400 to-zinc-600 bg-clip-text text-transparent">
						<br className="hidden lg:visible" />
						Available on iOS and Android
					</span>
				</h1>
				<p className="w-full max-w-[600px] text-center text-ink-faint">
					Using the mobile app, you can sync your files across all your devices. Take your
					personal data with you wherever you are!
				</p>
			</div>
		</div>
	);
};

export default Mobile;
