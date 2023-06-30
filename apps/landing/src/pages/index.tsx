import Head from 'next/head';
import AppEmbed from '~/components/AppEmbed';
import HomeCTA from '~/components/HomeCTA';
import NewBanner from '~/components/NewBanner';
import PageWrapper from '~/components/PageWrapper';
import Space from '~/components/Space';

export default function HomePage() {
	return (
		<PageWrapper>
			<div className="flex w-full flex-col items-center px-4">
				<Head>
					<title>Spacedrive — A file manager from the future.</title>
					<meta
						name="description"
						content="Combine your drives and clouds into one database that you can organize and explore from any device. Designed for creators, hoarders and the painfully disorganized."
					/>
					<meta
						property="og:image"
						content="https://raw.githubusercontent.com/spacedriveapp/.github/main/profile/spacedrive_icon.png"
					/>
					<meta
						name="keywords"
						content="files,file manager,spacedrive,file explorer,vdfs,distributed filesystem,cas,content addressable storage,virtual filesystem,photos app, video organizer,video encoder,tags,tag based filesystem"
					/>
					<meta name="author" content="Spacedrive Technology Inc." />
				</Head>
				<div className="mt-22 lg:mt-28" id="content" aria-hidden="true" />
				<div className="mt-24 lg:mt-8" />
				<NewBanner
					headline="Spacedrive raises $2M led by OSS Capital"
					href="/blog/spacedrive-funding-announcement"
					link="Read post"
				/>

				<h1 className="fade-in-heading z-30 mb-3 px-2 text-center text-4xl font-black leading-tight text-white md:text-7xl">
					One Explorer. All Your Files.
				</h1>
				<p className="animation-delay-1 fade-in-heading text-md leading-2 z-30 mb-8 mt-1 max-w-4xl text-center text-gray-450 lg:text-lg lg:leading-8">
					Unify files from all your devices and clouds into a single, easy-to-use
					explorer.
					<br />
					<span className="hidden sm:block">
						Designed for creators, hoarders and the painfully disorganized.
					</span>
				</p>
				<HomeCTA />
				<AppEmbed />
				{/* <AppEmbed /> */}
				<div className="h-[600px]" />
				<Space />
			</div>
		</PageWrapper>
	);
}
