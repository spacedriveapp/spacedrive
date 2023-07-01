/* eslint-disable jsx-a11y/alt-text */
import { Ball, Folder, Laptop, Mobile, Server } from '@sd/assets/icons';
import { Dropbox, GoogleDrive, Mega, iCloud } from '@sd/assets/images';
import Head from 'next/head';
import Image from 'next/image';
import { tw } from '@sd/ui';
import AppEmbed from '~/components/AppEmbed';
import HomeCTA from '~/components/HomeCTA';
import NewBanner from '~/components/NewBanner';
import PageWrapper from '~/components/PageWrapper';
import Space from '~/components/Space';

const ExplainerHeading = tw.h1`z-30 mb-3 px-2 text-center text-3xl font-black leading-tight text-white`;
const ExplainerText = tw.p`leading-2 z-30 mb-8 mt-1 max-w-4xl text-center text-gray-450"`;

const AppFrameOuter = tw.div`relative m-auto flex h-full w-full max-w-7xl rounded-lg border border-black transition-opacity`;
const AppFrameInner = tw.div`z-30 flex w-full rounded-lg border-t border-app-line/50 bg-app/30 backdrop-blur`;

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
				<div className="w-screen">
					<div className="relative mx-auto max-w-full sm:w-full sm:max-w-[1400px]">
						<div className="bloom burst bloom-one" />
						<div className="bloom burst bloom-three" />
						<div className="bloom burst bloom-two" />
					</div>
					<div className="z-30 mt-8 flex h-[255px] w-full px-6 sm:mt-20 sm:h-[428px] md:h-[428px] lg:h-[628px]">
						<AppFrameOuter>
							<AppFrameInner>
								<img className="rounded-lg" src="/images/test.png" />
							</AppFrameInner>
						</AppFrameOuter>
					</div>
				</div>
				{/* <AppEmbed /> */}
				<div className="relative mx-auto my-48 flex w-full max-w-4xl flex-col items-center justify-center px-4">
					<div className="absolute top-0 z-40 w-full py-16">
						<Image
							alt="icon"
							src={Mobile}
							width={100}
							className="absolute left-0 top-0 rotate-[20deg]"
						/>
						<Image
							alt="icon"
							src={Server}
							width={100}
							className="absolute left-[100px] top-0 rotate-[-40deg]"
						/>
						<Image
							alt="icon"
							src={Folder}
							width={70}
							className="absolute left-[170px] top-[90px] rotate-[10deg]"
						/>
						<Image
							alt="icon"
							src={Laptop}
							width={100}
							className="absolute left-[230px] top-0 rotate-[30deg]"
						/>
						<Image
							alt="icon"
							src={GoogleDrive}
							width={80}
							className="absolute right-0 top-0 rotate-[-30deg]"
						/>
						<Image
							alt="icon"
							src={iCloud}
							width={90}
							className="absolute right-[90px] top-0 rotate-[20deg]"
						/>
						<Image
							alt="icon"
							src={Dropbox}
							width={60}
							className="absolute right-[140px] top-[80px] rotate-[10deg]"
						/>
						<Image
							alt="icon"
							src={Mega}
							width={100}
							className="absolute right-[210px] top-0 rotate-[-40deg]"
						/>
					</div>
					<Image alt="ball" src={Ball} width={250} />
					<div className="relative mx-auto max-w-full sm:w-full sm:max-w-[1400px]">
						{/* <div className="bloom burst bloom-center" /> */}
					</div>
					<ExplainerHeading className="mt-5 text-6xl">
						A data hoarder's dream.
					</ExplainerHeading>
					<ExplainerText className="text-md lg:text-lg lg:leading-8">
						Spacedrive allows you to access and manage all files from one place, from
						any device or cloud service.
					</ExplainerText>
				</div>

				<div className="col relative mx-auto my-48 flex w-full max-w-4xl items-center justify-center gap-4 px-4">
					<AppFrameOuter>
						<AppFrameInner>
							<div className="p-5">
								<ExplainerHeading className="mt-5">
									A data hoarder's dream.
								</ExplainerHeading>
								<ExplainerText className="text-sm">
									Spacedrive allows you to access and manage all files from one
									place, from any device or cloud service.
								</ExplainerText>
							</div>
						</AppFrameInner>
					</AppFrameOuter>
					<AppFrameOuter>
						<AppFrameInner>
							<div className="p-5">Some content</div>
						</AppFrameInner>
					</AppFrameOuter>
				</div>

				<Space />
			</div>
		</PageWrapper>
	);
}
