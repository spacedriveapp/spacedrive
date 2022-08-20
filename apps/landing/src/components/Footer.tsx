import {
	Discord,
	Github,
	Instagram,
	Opencollective,
	Twitch,
	Twitter
} from '@icons-pack/react-simple-icons';
import React from 'react';

import AppLogo from '../assets/images/logo.png';

function FooterLink(props: { children: string | JSX.Element; link: string; blank?: boolean }) {
	return (
		<a
			href={props.link}
			target={props.blank ? '_blank' : ''}
			className="text-gray-300 hover:text-white"
			rel="noreferrer"
		>
			{props.children}
		</a>
	);
}

export function Footer() {
	return (
		<footer id="footer" className="z-50 w-screen pt-3 border-t border-gray-550 bg-gray-850">
			<div className="container grid grid-cols-2 gap-6 p-8 pt-10 pb-20 m-auto text-white min-h-64 sm:grid-cols-2 lg:grid-cols-6">
				<div className="col-span-2">
					<img src={AppLogo} className="w-10 h-10 mb-5" />

					<h3 className="mb-1 text-xl font-bold">Spacedrive</h3>
					<p className="text-sm text-gray-350">&copy; Copyright 2022 Spacedrive Technology Inc.</p>
					<div className="flex flex-row mt-6 mb-10 space-x-3">
						<FooterLink link="https://twitter.com/spacedriveapp">
							<Twitter />
						</FooterLink>
						<FooterLink link="https://discord.gg/gTaF2Z44f5">
							<Discord />
						</FooterLink>
						<FooterLink link="https://instagram.com/spacedriveapp">
							<Instagram />
						</FooterLink>
						<FooterLink link="https://github.com/spacedriveapp">
							<Github />
						</FooterLink>
						<FooterLink link="https://opencollective.com/spacedrive">
							<Opencollective />
						</FooterLink>
						<FooterLink link="https://twitch.tv/jamiepinelive">
							<Twitch />
						</FooterLink>
					</div>
				</div>

				<div className="flex flex-col col-span-1 space-y-2">
					<h3 className="mb-1 text-xs font-bold uppercase ">About</h3>

					<FooterLink link="/team">Team</FooterLink>
					<FooterLink link="/faq">FAQ</FooterLink>
					<FooterLink link="/careers">Careers</FooterLink>
					<FooterLink link="/changelog">Changelog</FooterLink>
					<FooterLink link="/blog">Blog</FooterLink>
				</div>
				<div className="flex flex-col col-span-1 space-y-2 pointer-events-none">
					<h3 className="mb-1 text-xs font-bold uppercase">Downloads</h3>
					<div className="flex flex-col col-span-1 space-y-2 opacity-50">
						<FooterLink link="#">macOS</FooterLink>
						<FooterLink link="#">Windows</FooterLink>
						<FooterLink link="#">Linux</FooterLink>
					</div>
				</div>
				<div className="flex flex-col col-span-1 space-y-2">
					<h3 className="mb-1 text-xs font-bold uppercase ">Developers</h3>
					<FooterLink blank link="https://github.com/spacedriveapp/spacedrive/tree/main/docs">
						Documentation
					</FooterLink>
					<FooterLink
						blank
						link="https://github.com/spacedriveapp/spacedrive/blob/main/CONTRIBUTING.md"
					>
						Contribute
					</FooterLink>
					<div className="opacity-50 pointer-events-none">
						<FooterLink link="#">Extensions</FooterLink>
					</div>
					<div className="opacity-50 pointer-events-none">
						<FooterLink link="#">Self Host</FooterLink>
					</div>
				</div>
				<div className="flex flex-col col-span-1 space-y-2">
					<h3 className="mb-1 text-xs font-bold uppercase ">Org</h3>
					<FooterLink blank link="https://opencollective.com/spacedrive">
						Open Collective
					</FooterLink>
					<FooterLink blank link="https://github.com/spacedriveapp/spacedrive/blob/main/LICENSE">
						License
					</FooterLink>
					<div className="opacity-50 pointer-events-none">
						<FooterLink link="#">Privacy</FooterLink>
					</div>
					<div className="opacity-50 pointer-events-none">
						<FooterLink link="#">Terms</FooterLink>
					</div>
				</div>
			</div>
		</footer>
	);
}
