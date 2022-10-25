import {
	AcademicCapIcon,
	BookOpenIcon,
	ChatBubbleOvalLeftIcon,
	MapIcon,
	UsersIcon
} from '@heroicons/react/24/solid';
import { Discord, Github } from '@icons-pack/react-simple-icons';
import AppLogo from '@sd/assets/images/logo.png';
import { Button, Dropdown } from '@sd/ui';
import clsx from 'clsx';
import { DotsThreeVertical } from 'phosphor-react';
import { PropsWithChildren, useEffect, useState } from 'react';
import * as router from 'vite-plugin-ssr/client/router';

import { positions } from '../pages/careers.page';
import { getWindow } from '../utils';

function NavLink(props: PropsWithChildren<{ link?: string }>) {
	return (
		<a
			href={props.link ?? '#'}
			target={props.link?.startsWith('http') ? '_blank' : undefined}
			className="p-4 text-gray-300 no-underline transition cursor-pointer hover:text-gray-50"
			rel="noreferrer"
		>
			{props.children}
		</a>
	);
}

function link(path: string) {
	return {
		selected: getWindow()?.location.href.includes(path),
		onClick: () => router.navigate(path)
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

	useEffect(() => {
		if (!window) return;
		setTimeout(onScroll, 0);
		getWindow()?.addEventListener('scroll', onScroll);
		return () => getWindow()?.removeEventListener('scroll', onScroll);
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	return (
		<div
			className={clsx(
				'fixed transition px-2 z-[55] w-full h-16 border-b ',
				isAtTop
					? 'bg-transparent border-transparent'
					: 'border-gray-550 bg-gray-700 bg-opacity-80 backdrop-blur'
			)}
		>
			<div className="relative flex max-w-[100rem] mx-auto items-center h-full m-auto p-5">
				<a href="/" className="absolute flex flex-row items-center">
					<img src={AppLogo} className="z-30 w-8 h-8 mr-3" />
					<h3 className="text-xl font-bold text-white">Spacedrive</h3>
				</a>

				<div className="hidden m-auto space-x-4 text-white lg:block ">
					<NavLink link="/roadmap">Roadmap</NavLink>
					<NavLink link="/team">Team</NavLink>
					<NavLink link="/blog">Blog</NavLink>
					<NavLink link="/docs/product/getting-started/introduction">Docs</NavLink>
					<div className="relative inline">
						<NavLink link="/careers">Careers</NavLink>
						{positions.length > 0 ? (
							<span className="absolute bg-opacity-80 px-[5px] text-xs rounded-md bg-primary -top-1 -right-2">
								{` ${positions.length} `}
							</span>
						) : null}
					</div>
				</div>
				<div className="flex-1 lg:hidden" />
				<Dropdown.Root
					button={
						<Button className="ml-[140px] hover:!bg-transparent" size="icon">
							<DotsThreeVertical weight="bold" className="w-6 h-6 " />
						</Button>
					}
					className="block h-6 text-white w-44 top-2 right-4 lg:hidden"
					itemsClassName="!rounded-2xl shadow-2xl shadow-black p-2 !bg-gray-850 mt-2 !border-gray-500 text-[15px]"
				>
					<Dropdown.Section>
						<Dropdown.Item
							icon={Github}
							onClick={redirect('https://github.com/spacedriveapp/spacedrive')}
						>
							Repository
						</Dropdown.Item>
						<Dropdown.Item icon={Discord} onClick={redirect('https://discord.gg/gTaF2Z44f5')}>
							Join Discord
						</Dropdown.Item>
					</Dropdown.Section>
					<Dropdown.Section>
						<Dropdown.Item icon={MapIcon} {...link('/roadmap')}>
							Roadmap
						</Dropdown.Item>
						<Dropdown.Item
							icon={BookOpenIcon}
							{...link('/docs/product/getting-started/introduction')}
						>
							Docs
						</Dropdown.Item>
						<Dropdown.Item icon={UsersIcon} {...link('/team')}>
							Team
						</Dropdown.Item>
						<Dropdown.Item icon={ChatBubbleOvalLeftIcon} {...link('/blog')}>
							Blog
						</Dropdown.Item>
						<Dropdown.Item icon={AcademicCapIcon} {...link('/careers')}>
							Careers
						</Dropdown.Item>
					</Dropdown.Section>
				</Dropdown.Root>

				<div className="absolute flex-row hidden space-x-5 right-3 lg:flex">
					<a href="https://discord.gg/gTaF2Z44f5" target="_blank" rel="noreferrer">
						<Discord className="text-white" />
					</a>
					<a href="https://github.com/spacedriveapp/spacedrive" target="_blank" rel="noreferrer">
						<Github className="text-white" />
					</a>
				</div>
			</div>
		</div>
	);
}
