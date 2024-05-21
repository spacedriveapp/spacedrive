import { Discord, Github } from '@sd/assets/svgs/brands';
import Image from 'next/image';
import Link from 'next/link';
import { PropsWithChildren } from 'react';

import { positions } from '../careers/data';
import Logo from '../logo.png';
import { MobileDropdown } from './MobileDropdown';

export function NavBar() {
	return (
		<div className="navbar-blur fixed z-[55] h-16 w-full !bg-black/10 px-2 transition">
			<div className="relative m-auto flex h-full max-w-[100rem] items-center p-5">
				<Link href="/" className="absolute flex flex-row items-center">
					<Image alt="Spacedrive logo" src={Logo} className="z-30 mr-3 size-8" />
					<h3 className="text-xl font-bold text-white">Spacedrive</h3>
				</Link>

				<div className="m-auto hidden space-x-4 text-white lg:block ">
					<NavLink link="/roadmap">Roadmap</NavLink>
					<NavLink link="/team">Team</NavLink>
					{/* <NavLink link="/pricing">Pricing</NavLink> */}
					<NavLink link="/blog">Blog</NavLink>
					<NavLink link="/docs/product/getting-started/introduction">Docs</NavLink>
					<div className="relative inline">
						<NavLink link="/careers">Careers</NavLink>
						{positions.length > 0 ? (
							<span className="absolute -right-2 -top-1 rounded-md bg-primary/80 px-[5px] text-xs">
								{` ${positions.length} `}
							</span>
						) : null}
					</div>
				</div>
				<div className="flex-1 lg:hidden" />
				<MobileDropdown />
				<div className="absolute right-3 hidden flex-row space-x-5 lg:flex">
					<Link
						aria-label="discord"
						href="https://discord.gg/gTaF2Z44f5"
						target="_blank"
						rel="noreferrer"
					>
						<Discord className="size-6 text-white opacity-100 duration-300 hover:opacity-50" />
					</Link>
					<Link
						aria-label="github"
						href="https://github.com/spacedriveapp/spacedrive"
						target="_blank"
						rel="noreferrer"
					>
						<Github className="size-6 text-white opacity-100 duration-300 hover:opacity-50" />
					</Link>
				</div>
			</div>
			<div className="absolute bottom-0 flex h-1 w-full flex-row items-center justify-center pt-4 opacity-100">
				<div className="h-px w-1/2 bg-gradient-to-r from-transparent to-white/10"></div>
				<div className="h-px w-1/2 bg-gradient-to-l from-transparent to-white/10"></div>
			</div>
		</div>
	);
}

function NavLink(props: PropsWithChildren<{ link?: string }>) {
	return (
		<Link
			href={props.link ?? '#'}
			target={props.link?.startsWith('http') ? '_blank' : undefined}
			className="cursor-pointer p-4 text-[11pt] text-gray-300 no-underline transition hover:text-gray-50"
			rel="noreferrer"
		>
			{props.children}
		</Link>
	);
}
