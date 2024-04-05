import { Book, Chat, DotsThreeVertical, MapPin, User } from '@phosphor-icons/react/dist/ssr';
import { Academia, Discord, Github } from '@sd/assets/svgs/brands';
import clsx from 'clsx';
import Image from 'next/image';
import Link from 'next/link';
import { NextRouter, useRouter } from 'next/router';
import { PropsWithChildren, useEffect, useState } from 'react';
import { Button, Dropdown } from '@sd/ui';
import { positions } from '~/app/careers/data';
import { getWindow } from '~/utils/util';

import Logo from '../../public/logo.png';

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

function link(path: string, router: NextRouter) {
	const selected = getWindow()?.location.href.includes(path);

	return {
		selected,
		onClick: () => router.push(path),
		className: clsx(selected && 'bg-accent/20')
	};
}

function redirect(href: string) {
	return () => (window.location.href = href);
}

export default function NavBar() {
	const [isAtTop, setIsAtTop] = useState(true);
	const window = getWindow();

	function onScroll() {
		if ((getWindow()?.pageYOffset || 0) < 20) setIsAtTop(true);
		else if (isAtTop) setIsAtTop(false);
	}

	const router = useRouter();

	useEffect(() => {
		if (!window) return;
		setTimeout(onScroll, 0);
		getWindow()?.addEventListener('scroll', onScroll);
		return () => getWindow()?.removeEventListener('scroll', onScroll);
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

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
				<Dropdown.Root
					button={
						<Button
							aria-label="mobile-menu"
							className="ml-[140px] hover:!bg-transparent"
							size="icon"
						>
							<DotsThreeVertical weight="bold" className="size-6 " />
						</Button>
					}
					className="right-4 top-2 block h-6 w-44 text-white lg:hidden"
					itemsClassName="!rounded-2xl shadow-2xl shadow-black p-2 !bg-gray-850 mt-2 !border-gray-500 text-[15px]"
				>
					<Dropdown.Section>
						<Dropdown.Item
							icon={Discord}
							onClick={redirect('https://discord.gg/gTaF2Z44f5')}
						>
							Join Discord
						</Dropdown.Item>
						<Dropdown.Item
							icon={Github}
							onClick={redirect('https://github.com/spacedriveapp/spacedrive')}
						>
							Repository
						</Dropdown.Item>
					</Dropdown.Section>
					<Dropdown.Section>
						<Dropdown.Item icon={MapPin} {...link('/roadmap', router)}>
							Roadmap
						</Dropdown.Item>
						<Dropdown.Item icon={User} {...link('/team', router)}>
							Team
						</Dropdown.Item>
						{/* <Dropdown.Item icon={Money} {...link('/pricing', router)}>
							Pricing
						</Dropdown.Item> */}
						<Dropdown.Item icon={Chat} {...link('/blog', router)}>
							Blog
						</Dropdown.Item>
						<Dropdown.Item
							icon={Book}
							{...link('/docs/product/getting-started/introduction', router)}
						>
							Docs
						</Dropdown.Item>
						<Dropdown.Item icon={Academia} {...link('/careers', router)}>
							Careers
							{positions.length > 0 ? (
								<span className="ml-2 rounded-md bg-primary px-[5px] py-px text-xs">
									{positions.length}
								</span>
							) : null}
						</Dropdown.Item>
					</Dropdown.Section>
				</Dropdown.Root>

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
