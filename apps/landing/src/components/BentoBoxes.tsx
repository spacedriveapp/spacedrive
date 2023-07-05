import { EncryptedLock } from '@sd/assets/icons';
import clsx from 'clsx';
import Image from 'next/image';
import { tw } from '@sd/ui';

const Heading = tw.h1`z-30 mb-3 px-2 text-center font-black leading-tight text-white text-4xl`;
const Text = tw.p`leading-2 text-ink-dull z-30 mb-8 mt-1 max-w-4xl text-center text-md lg:text-lg lg:leading-8"`;

// @ts-expect-error
const BentoBox = ({ rowSpan = 1, colSpan = 1, className = '', children }) => (
	<div
		style={{ gridRow: `span ${rowSpan}`, gridColumn: `span ${colSpan}` }}
		className={clsx(
			`block overflow-hidden rounded-lg border border-app-line/80 bg-app-box/60 backdrop-blur`,
			className
		)}
	>
		{children}
	</div>
);

export const BentoBoxes = () => {
	return (
		<div className="mb-48 grid w-full max-w-7xl auto-rows-[300px] grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4">
			<div className="absolute inset-x-0">
				<div className="relative mx-auto max-w-full sm:w-full sm:max-w-[1400px]">
					<div className="bloom burst bloom-three left-1 top-[500px]" />
					<div className="bloom burst bloom-two right-1 top-[900px]" />
					<div className="bloom burst bloom-one" />
				</div>
			</div>
			<BentoBox colSpan={2} className="p-6">
				<Heading className="pt-4">Cross platform</Heading>
				<Text>
					macOS, Windows, Linux, iOS, Android, and the web. Spacedrive is everywhere.
				</Text>
			</BentoBox>
			<BentoBox rowSpan={2} className="p-6">
				<Heading className="pt-4 !text-left">Light as a feather</Heading>
				<Text className="!text-left">
					Spacedrive is built without the bloat. It's fast, native feeling and doesn't hog
					your resources.
				</Text>
			</BentoBox>
			<BentoBox className="p-6">
				<Image alt="EncryptedLock" src={EncryptedLock} width={150} className="mx-auto" />
				<Heading className="text-xl font-bold">Native encryption</Heading>
				<Text className="!text-sm">
					Encryption tools and a key manager make Spacedrive a safe haven for your
					sensitive data.
				</Text>
			</BentoBox>

			<BentoBox className="p-6">
				<Heading className="text-xl font-bold">Free & open source</Heading>
			</BentoBox>
			<BentoBox rowSpan={2}>
				<div className="p-6">
					<Heading className="text-xl font-bold">Packed with tools</Heading>
					<Text className="!text-sm">
						Everything you need from a file manager and much more...
					</Text>
				</div>
				<div className="circle -mt-10">
					<Image
						alt="Context menu"
						width={612}
						height={1004}
						src="/images/contextmenu.png"
						className="mx-auto"
					/>
				</div>
			</BentoBox>
			<BentoBox className="p-6">
				<Heading className="pt-4 !text-left text-[18pt] font-bold">
					File types, <br />
					we know them all.
				</Heading>
			</BentoBox>
			<BentoBox>
				<div className="p-6">
					<Heading className="text-xl font-bold">Quick in-app preview</Heading>
					<Text className="!text-sm">
						Preview images, video, text, PDFs and more without opening them in another
						app.
					</Text>
				</div>
				<div className="circle -mt-16 ">
					<Image
						alt="Quick preview"
						width={500}
						height={500}
						src="/images/quickpreview.png"
						className="mx-auto"
					/>
				</div>
			</BentoBox>
			<BentoBox colSpan={2} rowSpan={2} className="p-6">
				<Heading className="text-5xl font-bold">All the views</Heading>
			</BentoBox>

			<BentoBox colSpan={2} className="p-6">
				<Heading className="text-3xl font-bold">Privacy means local first</Heading>
			</BentoBox>
			<BentoBox colSpan={2} rowSpan={2} className="p-6">
				<Heading className="text-5xl font-bold">Supercharged tags</Heading>
			</BentoBox>
			<BentoBox className="p-6">
				<Heading className="text-xl font-bold">Content addressable storage</Heading>
			</BentoBox>
			<BentoBox className="p-6">
				<Heading className="text-xl font-bold">Preview media generation</Heading>
			</BentoBox>

			<BentoBox className="p-6" colSpan={2}>
				<Heading className="text-xl font-bold">Peer-to-peer</Heading>
			</BentoBox>
			<BentoBox colSpan={4} className="p-6">
				<Heading className="p-4 !text-left text-5xl font-bold">Spacedrop</Heading>
			</BentoBox>
		</div>
	);
};
