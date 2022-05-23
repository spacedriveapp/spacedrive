import {
	ClockIcon,
	CogIcon,
	HeartIcon,
	LockClosedIcon,
	MapIcon,
	QuestionMarkCircleIcon
} from '@heroicons/react/solid';
import { Discord, Github } from '@icons-pack/react-simple-icons';
import { Button, Dropdown } from '@sd/ui';
import clsx from 'clsx';
import { Link, List, MapPin, Question } from 'phosphor-react';
import React, { useEffect, useState } from 'react';

import { ReactComponent as AppLogo } from '../assets/app-logo.svg';

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

export default function NavBar() {
	const [isAtTop, setIsAtTop] = useState(window.pageYOffset < 20);

	function onScroll(event: Event) {
		if (window.pageYOffset < 20) setIsAtTop(true);
		else if (isAtTop) setIsAtTop(false);
	}

	useEffect(() => {
		window.addEventListener('scroll', onScroll);
		return () => window.removeEventListener('scroll', onScroll);
	}, []);

	return (
		<div
			className={clsx(
				'fixed transition z-40 w-full h-16 border-b ',
				isAtTop
					? 'bg-transparent border-transparent'
					: 'border-gray-550 bg-gray-750 bg-opacity-80 backdrop-blur'
			)}
		>
			<div className="container relative flex items-center h-full px-5 m-auto">
				<a href="/" className="absolute flex flex-row items-center">
					<AppLogo className="z-30 w-8 h-8 mr-3" />
					<h3 className="text-xl font-bold text-white">
						Spacedrive
						{/* <span className="ml-2 text-xs text-gray-400 uppercase">ALPHA</span> */}
					</h3>
				</a>

				<div className="hidden m-auto space-x-4 text-white lg:block ">
					<NavLink link="/roadmap">Roadmap</NavLink>
					<NavLink link="/faq">FAQ</NavLink>
					<NavLink link="/team">Team</NavLink>
					<NavLink link="https://spacedrive.hashnode.dev">Blog</NavLink>
					{/* <NavLink link="/change-log">Changelog</NavLink>
          <NavLink link="/privacy">Privacy</NavLink> */}
					<NavLink link="https://opencollective.com/spacedrive">Sponsor us</NavLink>
				</div>
				<Dropdown
					className="absolute block h-6 w-44 top-2 right-4 lg:hidden"
					items={[
						[
							{
								name: 'Repository',
								icon: Github,
								onPress: () =>
									(window.location.href = 'https://github.com/spacedriveapp/spacedrive')
							},
							{
								name: 'Join Discord',
								icon: Discord,
								onPress: () => (window.location.href = 'https://discord.gg/gTaF2Z44f5')
							}
						],
						[
							{
								name: 'Roadmap',
								icon: MapIcon,
								onPress: () => (window.location.href = '/roadmap'),
								selected: window.location.href.includes('/roadmap')
							},
							{
								name: 'FAQ',
								icon: QuestionMarkCircleIcon,
								onPress: () => (window.location.href = '/faq'),
								selected: window.location.href.includes('/faq')
							},
							// {
							//   name: 'Changelog',
							//   icon: ClockIcon,
							//   onPress: () => (window.location.href = '/changelog'),
							//   selected: window.location.href.includes('/changelog')
							// },
							// {
							//   name: 'Privacy',
							//   icon: LockClosedIcon,
							//   onPress: () => (window.location.href = '/privacy'),
							//   selected: window.location.href.includes('/privacy')
							// },
							{
								name: 'Sponsor us',
								icon: HeartIcon,
								onPress: () => (window.location.href = 'https://opencollective.com/spacedrive')
							}
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
