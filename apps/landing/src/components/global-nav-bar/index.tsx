'use client';

import { List, X } from '@phosphor-icons/react/dist/ssr';
import { AnimatePresence, motion } from 'framer-motion';
import Image from 'next/image';
import Link from 'next/link';
import { PropsWithChildren, useState } from 'react';
import appFullLogo from '~/assets/app_full_logo.svg?url';
import { DownloadButton } from '~/components/cta-buttons/download-button';
import { useCurrentPlatform } from '~/components/cta-buttons/use-current-platform';

import '~/styles/navbar.css';

export function NavBar() {
	const currentPlatform = useCurrentPlatform();
	const [isMenuOpen, setIsMenuOpen] = useState(false);

	return (
		<>
			{/* Main Navbar */}
			<motion.nav
				className="fixed z-[110] w-full p-4"
				initial={{ opacity: 1 }}
				animate={{ opacity: isMenuOpen ? 0 : 1 }}
				transition={{ duration: 0.2 }}
			>
				<div
					className="flex w-full min-w-[300px] items-center justify-between rounded-[10px] px-[24px] py-[12px] shadow-[0px_-10px_20px_0px_rgba(40,134,213,0.05)]"
					style={{
						backgroundImage: `url('images/misc/NoisePattern.png')`,
						backgroundColor: '#141419',
						backgroundPosition: '0% 0%',
						backgroundSize: '50px 50px',
						backgroundRepeat: 'repeat',
						backgroundBlendMode: 'overlay, normal',
						border: '1px rgba(30, 30, 38, 0.00)'
					}}
				>
					{/* Spacedrive Logo and Links */}
					<div className="flex items-center gap-[20px]">
						<Link href="/" className="flex flex-row items-center gap-1">
							<Image
								alt="Spacedrive"
								src={appFullLogo}
								width={200}
								height={55}
								className="z-30 mr-[6px] h-[3.5rem] w-auto"
							/>
						</Link>
						<div className="hidden items-center gap-[10px] whitespace-nowrap xl:flex">
							<NavLink link="/explorer">Explorer</NavLink>
							<NavLink link="/cloud">Cloud</NavLink>
							<NavLink link="/teams">Teams</NavLink>
							<NavLink link="/assistant">
								Assistant <NewIcon />
							</NavLink>
							<NavLink link="/store">Store</NavLink>
							<NavLink link="/use-cases">Use Cases</NavLink>
							<NavLink link="/blog">Blog</NavLink>
							<NavLink link="/docs/product/getting-started/introduction">
								Docs
							</NavLink>
						</div>
					</div>

					{/* Download Button */}
					<div className="hidden items-center gap-[20px] xl:flex">
						<DownloadButton
							name={currentPlatform?.name ?? 'macOS'}
							link={`https://spacedrive.com/api/releases/desktop/stable/${currentPlatform?.os}/x86_64`}
						/>
					</div>

					{/* List Icon */}
					<div className="flex items-center gap-[20px] xl:hidden">
						<motion.button
							className="block"
							onClick={() => setIsMenuOpen(!isMenuOpen)}
							whileTap={{ rotate: isMenuOpen ? -180 : 180 }}
						>
							<List className="size-6 text-white" />
						</motion.button>
					</div>
				</div>
			</motion.nav>

			{/* Slide-Out Navbar */}
			<AnimatePresence>
				{isMenuOpen && (
					<>
						{/* Background Overlay */}
						<motion.div
							initial={{ opacity: 0 }}
							animate={{ opacity: 0.5 }}
							exit={{ opacity: 0 }}
							className="fixed left-0 top-0 z-[115] size-full bg-black"
							onClick={() => setIsMenuOpen(false)}
						/>

						{/* Slide-Out Panel */}
						<motion.div
							initial={{ x: '-100%' }}
							animate={{ x: 0 }}
							exit={{ x: '-100%' }}
							transition={{ type: 'spring', stiffness: 300, damping: 30 }}
							className="fixed left-0 top-0 z-[120] h-full w-64 bg-[#141419] p-4 shadow-lg"
						>
							{/* Close Button */}
							<div className="flex justify-end">
								<motion.button
									className="block"
									onClick={() => setIsMenuOpen(false)}
									whileTap={{ rotate: -90 }}
								>
									<X className="size-8 pt-2 text-white" />
								</motion.button>
							</div>

							{/* Nav Links */}
							<div className="flex flex-col items-start space-y-4 p-4">
								<NavLink link="/explorer">Explorer</NavLink>
								<NavLink link="/cloud">Cloud</NavLink>
								<NavLink link="/teams">Teams</NavLink>
								<NavLink link="/assistant">
									Assistant <NewIcon />
								</NavLink>
								<NavLink link="/store">Store</NavLink>
								<NavLink link="/use-cases">Use Cases</NavLink>
								<NavLink link="/blog">Blog</NavLink>
								<NavLink link="/docs/product/getting-started/introduction">
									Docs
								</NavLink>
								<DownloadButton
									name={currentPlatform?.name ?? 'macOS'}
									link={`https://spacedrive.com/api/releases/desktop/stable/${currentPlatform?.os}/x86_64`}
								/>
							</div>
						</motion.div>
					</>
				)}
			</AnimatePresence>
		</>
	);
}

function NavLink(props: PropsWithChildren<{ link?: string }>) {
	return (
		<Link
			href={props.link ?? '#'}
			target={props.link?.startsWith('http') ? '_blank' : undefined}
			className="inline-flex cursor-pointer items-center justify-center gap-[6px] p-4 text-[11pt] text-gray-300 no-underline transition hover:text-gray-50"
			rel="noreferrer"
		>
			{props.children}
		</Link>
	);
}

function NewIcon() {
	return (
		<div className="flex items-center justify-center rounded-[4px] bg-[#3397EB] px-1 py-[0.01rem] text-xs">
			NEW
		</div>
	);
}
