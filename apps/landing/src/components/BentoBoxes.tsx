import clsx from 'clsx';
import Image from 'next/image';
import Link from 'next/link';
import { Button, tw } from '@sd/ui';

const Heading = tw.h1`z-30 text-center font-semibold leading-tight text-white text-lg`;
const Text = tw.p`leading-2 text-ink-faint z-30 mb-8 mt-1 max-w-4xl text-center text-[14px] lg:leading-8"`;

interface Props {
	rowSpan?: number;
	colSpan?: number;
	className?: string;
	bgUrl?: string;
	children?: React.ReactNode;
}

const BentoBox = ({ rowSpan = 1, colSpan = 1, className = '', children, bgUrl = '' }: Props) => (
	<div
		style={{
			gridRow: `span ${rowSpan}`,
			gridColumn: `span ${colSpan}`,
			backgroundImage: `url('${bgUrl}')`,
			backgroundSize: 'cover',
			backgroundPosition: 'center',
			backgroundRepeat: 'no-repeat'
		}}
		className={clsx(
			`bento-box-border-gradient relative
			 overflow-hidden rounded-md bg-[#0e0e16] p-4 backdrop-blur`,
			className
		)}
	>
		<div className="absolute-center z-10 h-[50%] w-[560px] rounded-full bg-[#4e3772] opacity-50 blur-[173px]" />
		{children}
	</div>
);

// const AppFrameOuter = tw.div`relative m-auto flex w-full max-w-7xl rounded-lg border border-black transition-opacity`;
// const AppFrameInner = tw.div`z-30 flex w-full rounded-lg border-t border-app-line/50 bg-app/30 backdrop-blur`;

const BentoBoxes = () => {
	return (
		<div className="mt-[100px] flex w-full max-w-7xl auto-rows-[420px] flex-col gap-4 md:mt-[220px] lg:grid lg:grid-cols-6">
			<BentoBox colSpan={4} className="p-6" bgUrl="images/bento/encrypt-bg.webp">
				<>
					<Heading>Encryption</Heading>
					<Text className="mx-auto max-w-[417px]">
						Your files and folders are fully encrypted through our algorithm, preventing
						unauthorized access and guaranteed protection.
					</Text>
					<Image
						className="mx-auto"
						alt="Context menu"
						width={200}
						height={300}
						quality={100}
						src="/images/bento/lock.webp"
					/>
				</>
			</BentoBox>
			<BentoBox colSpan={2} className="p-6">
				<>
					<Image
						className="mx-auto mt-6"
						alt="Context menu"
						width={280}
						height={230}
						src="/images/bento/tags.webp"
					/>
					<div
						className="absolute right-0 top-0 z-30 h-full
					 w-full bg-gradient-to-r from-[#15161f00] from-50% to-[#12131e] to-80%"
					/>
					<div className="relative z-[40]">
						<Heading className="mt-[40px]">Powerful tags</Heading>
						<Text>
							Create and apply tags to your files and folders, and instantly locate
							desired content through filterable tags.
						</Text>
					</div>
				</>
			</BentoBox>
			<BentoBox colSpan={2} className="p-6" bgUrl="images/bento/search-grid.webp">
				<>
					<div className="relative z-30">
						<Heading>Search everything</Heading>
						<Text className="mx-auto max-w-[417px]">
							Easily find your files and folders through our search
						</Text>
					</div>
					<div
						className="absolute right-0 top-0 z-10 h-full
					 w-full bg-gradient-to-r from-[#15161f00] from-50% to-[#12131e] to-90%"
					/>
					<div
						className="absolute right-0 top-0 z-20 h-[80px]
					 w-full bg-[#12131e] blur-[20px]"
					/>
					<Image
						className="mx-auto"
						alt="Context menu"
						width={340}
						height={300}
						quality={100}
						src="/images/bento/search.webp"
					/>
				</>
			</BentoBox>
			<BentoBox colSpan={2} className="p-6">
				<>
					<div
						className="absolute right-0 top-0 z-10 h-full
					 w-full bg-gradient-to-r from-[#15161f00] from-50% to-[#12131e] to-90%"
					/>
					<div
						className="absolute bottom-0 right-0 z-20 h-[100px]
					 w-full bg-[#12131e] blur-[20px]"
					/>
					<Image
						className="mx-auto"
						alt="Context menu"
						width={340}
						height={300}
						src="/images/bento/library.webp"
					/>
					<div className="relative z-30 mt-[40px]">
						<Heading>Full Ownership & Control</Heading>
						<Text className="mx-auto">Make Spacedrive yours</Text>
					</div>
				</>
			</BentoBox>
			<BentoBox colSpan={2} className="p-6">
				<>
					<div className="relative z-30">
						<Heading>Spacedrop</Heading>
						<Text className="mx-auto max-w-[417px]">
							Send files to other devices quickly and easily
						</Text>
					</div>
					<div className="before-bento-gradient-border bento-radial-gradient-fade absolute right-0 top-0 z-20 h-full w-full" />
					<Image
						className="mx-auto"
						alt="Context menu"
						width={311}
						height={300}
						src="/images/bento/spacedrop.webp"
					/>
				</>
			</BentoBox>
			<BentoBox
				colSpan={3}
				className="h-[354px] p-6 lg:h-auto"
				bgUrl="images/bento/opensource-bg.webp"
			>
				<>
					<div className="relative z-30">
						<Heading>Free & Opensource</Heading>
						<Text className="mx-auto">
							Developers and users can contribute with new ideas and features
						</Text>
					</div>
					<div
						className="absolute right-0 top-0 z-10 h-full
					 w-[99%] bg-gradient-to-r from-[#15161f00] from-50% to-[#12131e] to-90%"
					/>
					<div
						className="before-bento-gradient-border absolute right-0 top-[0.05px] z-20 h-full
					 w-full bg-gradient-to-t from-[#15161f00] from-50% to-[#12131e] to-90%"
					/>
					<Link target="_blank" href="https://github.com/spacedriveapp/spacedrive">
						<Button
							size="lg"
							className="contribute-drop-shadow absolute-center relative z-40 block cursor-pointer border-0 bg-gradient-to-r
							 from-emerald-400 to-cyan-500 text-sm text-black !transition-all !duration-200 hover:opacity-80"
						>
							{`<>`} Contribute
						</Button>
					</Link>
				</>
			</BentoBox>
			<BentoBox colSpan={3} bgUrl={'/images/bento/platforms-bg.webp'} className="p-6">
				<>
					<video
						className="pointer-events-none relative top-[10px] z-30 mx-auto w-[70%]"
						autoPlay
						src="/images/bento/platforms.webm"
						playsInline
						muted
						loop
					/>
					<div className="relative z-30 mt-8">
						<Heading>Cross platform</Heading>
						<Text className="mx-auto max-w-[417px]">
							Windows, macOS, Linux, iOS, Android, and the web. Spacedrive is
							everywhere.
						</Text>
					</div>
					<div className="before-bento-gradient-border bento-radial-gradient-fade absolute right-0 top-0 z-20 h-full w-full" />
				</>
			</BentoBox>
		</div>
	);
};

export default BentoBoxes;
