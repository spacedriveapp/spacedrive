import dynamic from 'next/dynamic';
import Image from 'next/image';
import Link from 'next/link';
import { useMemo } from 'react';
import { Button, tw } from '@sd/ui';
import { useWindowSize } from '~/hooks/useWindowSize';

import { MagicCard, MagicContainer } from './MagicCard';
import PlatformsArt from './PlatformsArt';
import SpacedropArt from './SpacedropArt';

const Heading = tw.h1`z-30 text-center font-semibold leading-tight text-white text-lg`;
const Text = tw.p`leading-2 text-zinc-500 z-30 mb-8 mt-1 max-w-4xl text-center text-[14px] lg:leading-8"`;

interface Props {
	rowSpan?: number;
	colSpan?: number;
	className?: string;
	bgUrl?: string;
	children?: React.ReactNode;
}

const BentoBox = ({ rowSpan = 1, colSpan = 1, className = '', children, bgUrl = '' }: Props) => (
	<div
		className="rounded-[12px] border border-white/10"
		style={{
			gridRow: `span ${rowSpan}`,
			gridColumn: `span ${colSpan}`,
			backgroundImage: `url('${bgUrl}')`,
			backgroundSize: 'cover',
			backgroundPosition: 'center',
			backgroundRepeat: 'no-repeat',
			height: '420px'
		}}
	>
		<MagicCard className={className}>{children}</MagicCard>
	</div>
);

// const AppFrameOuter = tw.div`relative m-auto flex w-full max-w-7xl rounded-lg border border-black transition-opacity`;
// const AppFrameInner = tw.div`z-30 flex w-full rounded-lg border-t border-app-line/50 bg-app/30 backdrop-blur`;

const GitHubButton = dynamic(() => import('react-github-btn'), { ssr: false });

const BentoBoxes = () => {
	const { width } = useWindowSize();
	const particleCount = useMemo(() => {
		if (width) {
			return width > 768 ? 50 : 25;
		}
		return 50;
	}, [width]);
	return (
		<MagicContainer
			className="flex h-fit w-full max-w-7xl auto-rows-[420px] flex-col gap-4
		 lg:grid lg:grid-cols-6"
		>
			<BentoBox colSpan={4} className="p-6" bgUrl="images/bento/encrypt-bg.webp">
				<div className="bento-radial-gradient-fade absolute right-0 top-0 z-20 size-full" />
				<div className="relative z-20">
					<Heading>Encryption</Heading>
					<Text className="mx-auto max-w-[417px]">
						Your files and folders are fully encrypted through our algorithm, preventing
						unauthorized access and guaranteed protection.
					</Text>
				</div>
				<div className="flex h-4/5 w-auto items-start justify-center">
					<Image
						className="mx-auto"
						alt="Encryption"
						loading="lazy"
						width={200}
						height={300}
						quality={100}
						src="/images/bento/lock.webp"
					/>
				</div>
			</BentoBox>
			<BentoBox colSpan={2} className="p-6">
				<div className="flex h-3/4 w-auto items-center justify-center">
					<Image
						className="mx-auto mt-3 brightness-125"
						alt="Powerful tags"
						width={300}
						quality={100}
						loading="lazy"
						height={100}
						src="/images/bento/tags.webp"
					/>
				</div>
				<div className="bento-radial-gradient-fade absolute right-0 top-0 z-20 size-full" />

				<div className="relative z-40 mt-2 md:mt-7">
					<Heading>Powerful tags</Heading>
					<Text>
						Create and apply tags to your files and folders, and instantly locate
						desired content through filterable tags.
					</Text>
				</div>
			</BentoBox>
			<BentoBox colSpan={2} className="p-6">
				<div className="relative z-30">
					<Heading>Search everything</Heading>
					<Text className="mx-auto max-w-[417px]">
						Easily find your files and folders through our search
					</Text>
				</div>
				<div className="bento-radial-gradient-fade absolute right-0 top-0 z-20 size-full" />
				<div className="flex h-4/5 w-auto items-start justify-center">
					<Image
						className="mx-auto brightness-110"
						alt="Search"
						width={340}
						loading="lazy"
						height={300}
						quality={100}
						src="/images/bento/search.webp"
					/>
				</div>
			</BentoBox>
			<BentoBox colSpan={2} className="p-6">
				<div className="bento-radial-gradient-fade absolute right-0 top-0 z-20 size-full" />
				<div className="flex h-4/5 w-auto items-center justify-center">
					<Image
						className="mx-auto brightness-125"
						alt="Library"
						width={340}
						height={300}
						loading="lazy"
						quality={100}
						src="/images/bento/library.webp"
					/>
				</div>
				<div className="relative z-30 mt-[30px]">
					<Heading>Full Ownership & Control</Heading>
					<Text className="mx-auto">Make Spacedrive yours</Text>
				</div>
			</BentoBox>
			<BentoBox colSpan={2} className="p-6">
				<div className="relative z-30">
					<Heading>Spacedrop</Heading>
					<Text className="mx-auto max-w-[417px]">
						Send files to other devices quickly and easily
					</Text>
				</div>
				<div className="bento-radial-gradient-fade absolute right-0 top-0 z-20 size-full" />
				<div className="flex h-4/5 w-auto items-center justify-center">
					<div
						style={
							{
								'--floatduration': '4s'
							} as React.CSSProperties
						}
						className="floating"
					>
						<SpacedropArt />
					</div>
				</div>
			</BentoBox>
			<BentoBox
				colSpan={3}
				className="h-[354px] p-6 brightness-110 lg:h-auto"
				bgUrl="images/bento/opensource-bg.webp"
			>
				<div className="relative z-30">
					<Heading>Free & Opensource</Heading>
					<Text className="mx-auto">
						Developers and users can contribute with new ideas and features
					</Text>
				</div>
				<div className="bento-radial-gradient-fade absolute right-0 top-0 z-20 h-[420px] w-full" />
				<div className="absolute-center relative z-40 mt-[40px] md:mt-0">
					<Link target="_blank" href="https://github.com/spacedriveapp/spacedrive">
						<Button
							size="lg"
							className="contribute-drop-shadow mx-auto mb-4 block cursor-pointer border-0
							 bg-gradient-to-r from-emerald-400 to-cyan-500 text-sm text-black !transition-all !duration-200"
						>
							{`<>`} Contribute
						</Button>
					</Link>
					<GitHubButton
						href="https://github.com/spacedriveapp/spacedrive"
						data-size="large"
						data-show-count="true"
						aria-label="Star spacedriveapp/spacedrive on GitHub"
					>
						Star
					</GitHubButton>
				</div>
			</BentoBox>
			<BentoBox colSpan={3} className="relative p-6">
				<div
					style={
						{
							'--floatduration': '4s'
						} as React.CSSProperties
					}
					className="floating mx-auto flex
						h-[300px] w-full max-w-[500px]"
				>
					<PlatformsArt />
				</div>
				<div
					className="absolute-center h-[120px] w-[300px] bg-gradient-to-r
					from-fuchsia-500 from-10% to-blue-500 opacity-10 blur-[175px]"
				/>
				<div className="relative z-30">
					<Heading>Cross platform</Heading>
					<Text className="mx-auto max-w-[400px]">
						Windows, macOS, Linux, iOS, Android, and the web. Spacedrive is everywhere.
					</Text>
				</div>
				<div className="bento-radial-gradient-fade absolute right-0 top-0 z-20 size-full" />
			</BentoBox>
		</MagicContainer>
	);
};

export default BentoBoxes;
