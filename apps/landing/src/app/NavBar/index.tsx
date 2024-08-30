import { Cloud } from '@phosphor-icons/react/dist/ssr';
import { Discord, Github } from '@sd/assets/svgs/brands';
import Image from 'next/image';
import Link from 'next/link';
import { PropsWithChildren } from 'react';

import { positions } from '../careers/data';
import Logo from '../logo.png';
import { MobileDropdown } from './MobileDropdown';

export function NavBar() {
	return (
		// <div className="navbar-blur fixed z-[55] h-16 w-full !bg-black/10 px-2 transition">
		// 	<div className="relative m-auto flex h-full max-w-[100rem] items-center p-5">
		// 		<Link href="/" className="absolute flex flex-row items-center">
		// 			<Image alt="Spacedrive logo" src={Logo} className="z-30 mr-3 size-8" />
		// 			<h3 className="text-xl font-bold text-white">Spacedrive</h3>
		// 		</Link>

		// 		<div className="m-auto hidden space-x-4 text-white lg:block">
		// 			<NavLink link="/roadmap">Roadmap</NavLink>
		// 			<NavLink link="/team">Team</NavLink>
		// 			{/* <NavLink link="/pricing">Pricing</NavLink> */}
		// 			<NavLink link="/blog">Blog</NavLink>
		// 			<NavLink link="/docs/product/getting-started/introduction">Docs</NavLink>
		// 			<div className="relative inline">
		// 				<NavLink link="/careers">Careers</NavLink>
		// 				{positions.length > 0 ? (
		// 					<span className="absolute -right-2 -top-1 rounded-md bg-primary/80 px-[5px] text-xs">
		// 						{` ${positions.length} `}
		// 					</span>
		// 				) : null}
		// 			</div>
		// 		</div>
		// 		<div className="flex-1 lg:hidden" />
		// 		<MobileDropdown />
		// 		<div className="absolute right-3 hidden flex-row space-x-5 lg:flex">
		// 			<Link
		// 				aria-label="discord"
		// 				href="https://discord.gg/gTaF2Z44f5"
		// 				target="_blank"
		// 				rel="noreferrer"
		// 			>
		// 				<Discord className="size-6 text-white opacity-100 duration-300 hover:opacity-50" />
		// 			</Link>
		// 			<Link
		// 				aria-label="github"
		// 				href="https://github.com/spacedriveapp/spacedrive"
		// 				target="_blank"
		// 				rel="noreferrer"
		// 			>
		// 				<Github className="size-6 text-white opacity-100 duration-300 hover:opacity-50" />
		// 			</Link>
		// 		</div>
		// 	</div>
		// 	<div className="absolute bottom-0 flex h-1 w-full flex-row items-center justify-center pt-4 opacity-100">
		// 		<div className="h-px w-1/2 bg-gradient-to-r from-transparent to-white/10"></div>
		// 		<div className="h-px w-1/2 bg-gradient-to-l from-transparent to-white/10"></div>
		// 	</div>
		// </div>
		<nav className="fixed z-[100] w-full items-center justify-center p-10 transition">
			<div className="flex w-full content-between items-center rounded-[10px] border border-[#1e1e2600] bg-[#141419] px-[24px] py-[12px]">
				<div className="flex items-center gap-[26px]">
					<Link href="/" className="flex flex-row items-center">
						<Image
							alt="Spacedrive logo"
							src={Logo}
							className="z-30 mr-[6.7px] size-8"
						/>
						<h3 className="mr-[13.38] text-xl font-bold text-white">Spacedrive</h3>
					</Link>
					<div className="flex items-center gap-[10px]">
						<NavLink link="/explorer">Explorer</NavLink>
						<NavLink link="/cloud">Cloud</NavLink>
						<NavLink link="/teams">Teams</NavLink>
						<NavLink link="/assistant">
							Assistant <NewIcon />
						</NavLink>
						<NavLink link="/store">Store</NavLink>
						<NavLink link="/use-cases">Use Cases</NavLink>
						<NavLink link="/blog">Blog</NavLink>
						<NavLink link="/docs/product/getting-started/introduction">Docs</NavLink>
					</div>
				</div>
				<div className="flex-1" />
				<Link href="/">
					<button className="inline-flex items-center justify-center gap-[10px] rounded-xl bg-slate-800 p-2">
						<Cloud />
						<p>Cloud Login</p>
					</button>
				</Link>
			</div>
		</nav>
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
