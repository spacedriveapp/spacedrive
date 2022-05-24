import {
	Discord,
	Github,
	Instagram,
	Opencollective,
	Twitch,
	Twitter
} from '@icons-pack/react-simple-icons';
import React from 'react';

import { ReactComponent as AppLogo } from '../assets/app-logo.svg';

function FooterLink(props: { children: string | JSX.Element; link: string }) {
	return (
		<a href={props.link} target="_blank" className="text-gray-300 hover:text-white">
			{props.children}
		</a>
	);
}

export function Footer() {
	return (
		<footer id="footer" className="z-50 w-screen pt-3 border-t border-gray-550 bg-gray-850">
			<div className="container grid grid-cols-2 gap-6 p-8 pt-10 pb-20 m-auto text-white min-h-64 sm:grid-cols-2 lg:grid-cols-6">
				<div className="col-span-2">
					<AppLogo className="w-10 h-10 mb-5" />

					<h3 className="mb-1 text-xl font-bold">Spacedrive</h3>
					<p className="text-sm text-gray-350">&copy; Copyright 2022 Jamie Pine</p>
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
					<FooterLink link="https://github.com/spacedriveapp/spacedrive#motivation">
						Mission
					</FooterLink>
					<FooterLink link="/changelog">Changelog</FooterLink>
					<div className="opacity-50 pointer-events-none">
						<FooterLink link="#">Blog</FooterLink>
					</div>
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
					<FooterLink link="https://github.com/spacedriveapp/spacedrive/tree/main/docs">
						Documentation
					</FooterLink>
					<FooterLink link="https://github.com/spacedriveapp/spacedrive/tree/main/docs/developer/contributing.md">
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
					<FooterLink link="https://opencollective.com/spacedrive">Open Collective</FooterLink>
					<FooterLink link="https://github.com/spacedriveapp/spacedrive/blob/main/LICENSE">
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
