import { BookOpenIcon, MapIcon, QuestionMarkCircleIcon, UsersIcon } from '@heroicons/react/solid';
import { Discord, Github } from '@icons-pack/react-simple-icons';
import { Dropdown, DropdownItem } from '@sd/ui';
import clsx from 'clsx';
import { List } from 'phosphor-react';
import React, { useEffect, useState } from 'react';

import AppLogo from '../assets/images/logo.png';
import { positions } from '../pages/careers.page';
import { getWindow } from '../utils';

function NavLink(props: { link?: string; children: string }) {
	return (
		<a
			href={props.link ?? '#'}
			target={props.link?.startsWith('http') ? '_blank' : undefined}
			className="p-4 text-gray-300 no-underline transition cursor-pointer hover:text-gray-50"
		>
			{props.children}
		</a>
	);
}

function dropdownItem(
	props: { name: string; icon: any } & ({ href: string } | { path: string })
): DropdownItem[number] {
	if ('href' in props) {
		return {
			name: props.name,
			icon: props.icon,
			onPress: () => (window.location.href = props.href)
		};
	} else {
		return {
			name: props.name,
			icon: props.icon,
			onPress: () => (window.location.href = props.path),
			selected: getWindow()?.location.href.includes(props.path)
		};
	}
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
	}, []);

	return (
		<div
			className={clsx(
				'fixed transition-opacity z-[55] w-full h-16 border-b ',
				isAtTop
					? 'bg-transparent border-transparent'
					: 'border-gray-550 bg-gray-750 bg-opacity-80 backdrop-blur'
			)}
		>
			<div className="relative flex items-center h-full px-5 m-auto sm:container">
				<a href="/" className="absolute flex flex-row items-center">
					<img src={AppLogo} className="z-30 w-8 h-8 mr-3" />
					<h3 className="text-xl font-bold text-white">
						Spacedrive
						{/* <span className="ml-2 text-xs text-gray-400 uppercase">ALPHA</span> */}
					</h3>
				</a>

				<div className="hidden m-auto space-x-4 text-white lg:block ">
					<NavLink link="/roadmap">Roadmap</NavLink>
					<NavLink link="/faq">FAQ</NavLink>
					<NavLink link="/team">Team</NavLink>
					<NavLink link="/blog">Blog</NavLink>
					<div className="relative inline">
						<NavLink link="/careers">Careers</NavLink>
						{positions.length > 0 ? (
							<span className="absolute bg-opacity-80 px-[5px] text-xs rounded-md bg-primary -top-1 -right-2">
								{' '}
								{positions.length}{' '}
							</span>
						) : null}
					</div>
				</div>
				<Dropdown
					className="absolute block h-6 w-44 top-2 right-4 lg:hidden"
					items={[
						[
							dropdownItem({
								name: 'Repository',
								icon: Github,
								href: 'https://github.com/spacedriveapp/spacedrive'
							}),
							dropdownItem({
								name: 'Join Discord',
								icon: Discord,
								href: 'https://discord.gg/gTaF2Z44f5'
							})
						],
						[
							dropdownItem({
								name: 'Roadmap',
								icon: MapIcon,
								path: '/roadmap'
							}),
							dropdownItem({
								name: 'FAQ',
								icon: QuestionMarkCircleIcon,
								path: '/faq'
							}),
							dropdownItem({
								name: 'Team',
								icon: UsersIcon,
								path: '/team'
							}),
							dropdownItem({
								name: 'Blog',
								icon: BookOpenIcon,
								path: '/blog'
							}),
							dropdownItem({
								name: 'Careers',
								icon: QuestionMarkCircleIcon,
								path: '/careers'
							})
						]
					]}
					buttonIcon={<List weight="bold" className="w-6 h-6" />}
					buttonProps={{ className: '!p-1 ml-[140px]' }}
				/>
				<div className="absolute flex-row hidden space-x-5 right-3 lg:flex">
					<a href="https://discord.gg/gTaF2Z44f5" target="_blank">
						<Discord className="text-white" />
					</a>
					<a href="https://github.com/spacedriveapp/spacedrive" target="_blank">
						<Github className="text-white" />
					</a>
				</div>
			</div>
		</div>
	);
}
