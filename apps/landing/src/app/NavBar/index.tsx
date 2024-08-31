'use client';

import { ArrowCircleDown } from '@phosphor-icons/react/dist/ssr';
import { Discord, Github } from '@sd/assets/svgs/brands';
import Image from 'next/image';
import Link from 'next/link';
import { PropsWithChildren } from 'react';

import { positions } from '../careers/data';
import { DownloadButton } from '../Downloads/Button';
import { useCurrentPlatform } from '../Downloads/Platform';
import Logo from '../logo.png';
import { MobileDropdown } from './MobileDropdown';

export function NavBar() {
	const currentPlatform = useCurrentPlatform();

	return (
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
				<DownloadButton
					name={currentPlatform?.name ?? 'macOS'}
					link={`https://spacedrive.com/api/releases/desktop/stable/${currentPlatform?.os}/x86_64`}
				/>
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
