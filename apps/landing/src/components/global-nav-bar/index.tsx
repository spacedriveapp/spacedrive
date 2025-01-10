'use client';

import { List, X } from '@phosphor-icons/react';
import clsx from 'clsx';
import { AnimatePresence, motion } from 'framer-motion';
import Image from 'next/image';
import Link from 'next/link';
import { memo, ReactNode, useCallback, useState } from 'react';
import appFullLogo from '~/assets/app_full_logo.svg?url';
import { DownloadButton } from '~/components/download-button';

import '~/styles/navbar.css';

import { usePathname } from 'next/navigation';
import { useCurrentPlatform } from '~/utils/current-platform';
import { ExternalLinkRegex } from '~/utils/regex-external-link';

// If we had pages filled out, we could use this to generate the navigation items
// const NAVIGATION_ITEMS: { label: string; href: string; adornment?: string }[] = [
// 	{ label: 'Explorer', href: '/' },
// 	{ label: 'Cloud', href: '/cloud' },
// 	{ label: 'Teams', href: '/teams' },
// 	{ label: 'Assistant', href: '/assistant', adornment: 'New' },
// 	{ label: 'Store', href: '/store' },
// 	{ label: 'Use Cases', href: '/use-cases' },
// 	{ label: 'Blog', href: '/blog' },
// 	{ label: 'Docs', href: '/docs' }
// ];

const NAVIGATION_ITEMS: { label: string; href: string; adornment?: string }[] = [
	// { label: 'Explorer', href: '#' },

	{ label: 'Pricing', href: '/pricing' },
	// { label: 'Assistant', href: '#', adornment: 'New' },
	// { label: 'Store', href: '#' },
	// { label: 'Use Cases', href: '/use-cases' },
	{ label: 'Blog', href: '/blog' },
	{ label: 'Team', href: '/team' },
	{ label: 'Roadmap', href: '/roadmap' },
	{ label: 'Docs', href: '/docs/product/getting-started/introduction' },
	{ label: 'Careers', href: '/careers' }
];

const MemoizedNavBar = memo(function NavBar() {
	const [isMenuOpen, setIsMenuOpen] = useState(false);
	const currentPlatform = useCurrentPlatform();
	const pathname = usePathname();
	const isDocsPage = pathname.startsWith('/docs');

	const toggleMenu = useCallback(() => {
		setIsMenuOpen((prev) => !prev);
	}, []);

	return (
		<>
			{/* Main Navbar */}
			<motion.nav
				className={clsx(
					'to-primary-900/20',
					isDocsPage
						? 'fixed inset-x-0 top-0 z-[110] mx-auto mt-3 max-w-[100rem] px-6 sm:px-8'
						: 'fixed inset-x-0 top-0 z-[110] mx-auto mt-3 w-[calc(100%-2rem)] max-w-screen-xl sm:px-0',
					'overflow-hidden rounded-xl bg-sidebar/90 from-[#0E0E12]/90 from-50% shadow-[0px_-10px_20px_0px_rgba(40,134,213,0.05)] backdrop-blur will-change-transform'
				)}
				style={{
					border: '1px rgba(30, 30, 38, 0.00)',
					transform: 'translate3d(0,0,0)',
					backfaceVisibility: 'hidden'
				}}
				initial={{ opacity: 1 }}
				animate={{ opacity: isMenuOpen ? 0 : 1 }}
				transition={{ duration: 0.2, ease: 'easeOut' }}
			>
				{/* Gradient Borders */}
				<div className="absolute top-0 h-px w-full bg-gradient-to-r from-transparent via-[#2D2D37]/60 to-transparent" />
				<div className="absolute bottom-0 h-px w-full bg-gradient-to-r from-transparent via-[#2D2D37]/60 to-transparent" />
				{/* End of Gradient Borders */}
				<div
					className={clsx(
						'noise noise-faded noise-sm flex flex-nowrap items-center justify-between gap-x-8 overflow-hidden py-3',
						isDocsPage ? 'px-0' : 'px-6'
					)}
				>
					{/* Spacedrive Logo and Links */}
					<div className="flex items-center gap-[1.125rem]">
						<Link href="/">
							<Image
								alt="Spacedrive"
								src={appFullLogo}
								width={200}
								height={55}
								className="z-30 mr-[6px] h-14 w-auto select-none will-change-transform"
								style={{
									transform: 'translate3d(0,0,0)',
									backfaceVisibility: 'hidden'
								}}
							/>
						</Link>

						<div className="hidden items-center whitespace-nowrap lg:flex">
							{NAVIGATION_ITEMS.map(({ label, href, adornment }) => (
								<NavLink key={`nav-main-${label}-${href}`} href={href}>
									{label}{' '}
									{adornment && (
										<span className="inline-flex rounded bg-[#3397EB] px-1 py-0.5 align-middle text-xs font-bold uppercase leading-none text-white">
											{adornment}
										</span>
									)}
								</NavLink>
							))}
						</div>
					</div>

					{/* Download Button */}
					<div className="hidden items-center gap-2 lg:flex">
						<DownloadButton platform={currentPlatform} />
					</div>

					{/* List Icon */}
					<div className="flex items-center gap-[20px] lg:hidden">
						<motion.button
							className="block"
							onClick={toggleMenu}
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
							className="fixed left-0 top-0 z-[120] h-full w-72 overflow-auto bg-[#141419] p-4 shadow-lg"
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
							<div className="flex flex-col items-start space-y-2 p-4">
								{NAVIGATION_ITEMS.map(({ label, href, adornment }) => (
									<NavLink key={`nav-sub-${label}-${href}`} href={href}>
										{label}{' '}
										{adornment && (
											<span className="inline-flex rounded bg-[#3397EB] px-1 py-0.5 align-middle text-xs font-bold uppercase leading-none text-white">
												{adornment}
											</span>
										)}
									</NavLink>
								))}
								<div className="flex flex-col gap-4">
									<DownloadButton platform={currentPlatform} />
								</div>
							</div>
						</motion.div>
					</>
				)}
			</AnimatePresence>
		</>
	);
});

export { MemoizedNavBar as NavBar };

interface NavLinkProps {
	href: string;
	target?: string;
	children: ReactNode;
}

function NavLink({
	href,
	target = href.match(ExternalLinkRegex)?.length ? '_blank' : undefined,
	children
}: NavLinkProps) {
	return (
		<Link
			href={href ?? '#'}
			target={target}
			className="inline-flex cursor-pointer flex-row items-center justify-center gap-x-1.5 px-2.5 py-2 text-[14px] text-gray-300 no-underline transition hover:text-gray-50"
			rel="noreferrer"
		>
			{children}
		</Link>
	);
}
