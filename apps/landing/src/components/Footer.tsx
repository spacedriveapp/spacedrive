import { AppLogo } from '@sd/assets/images';
import {
	Discord,
	Github,
	Instagram,
	Opencollective,
	Twitch,
	Twitter
} from '@sd/assets/svgs/brands';
import Image from 'next/image';
import Link from 'next/link';
import { PropsWithChildren } from 'react';

function FooterLink(props: PropsWithChildren<{ link: string; blank?: boolean }>) {
	return (
		<Link
			href={props.link}
			target={props.blank ? '_blank' : ''}
			className="text-gray-300 duration-300 hover:text-white hover:opacity-50"
			rel="noreferrer"
			aria-label="link"
			{...props}
		>
			{props.children}
		</Link>
	);
}

export function Footer() {
	return (
		<footer id="footer" className="relative z-50 w-screen overflow-hidden pt-3 backdrop-blur">
			<div
				className="absolute bottom-0 left-0 z-[-1] h-[90px] w-[50%]
			bg-gradient-to-r from-violet-400 to-fuchsia-400 opacity-60 blur-[120px]"
			/>
			<div
				className="absolute right-0 top-[250px] z-[-1] h-[45%] w-full
			 bg-gradient-to-r from-transparent to-indigo-500 opacity-50 blur-[100px]"
			/>
			<div className="min-h-64 m-auto grid max-w-[100rem] grid-cols-2 gap-6 p-8 pb-20 pt-10 text-white sm:grid-cols-2 lg:grid-cols-6">
				<div className="col-span-2">
					<Image alt="Spacedrive logo" src={AppLogo} className="mb-5 h-10 w-10" />

					<h1 className="mb-1 text-xl font-bold">Spacedrive</h1>
					<p className="text-sm text-gray-350 opacity-50">
						&copy; Copyright {new Date().getFullYear()} Spacedrive Technology Inc.
					</p>
					<div className="mb-10 mt-12 flex flex-row space-x-3">
						<FooterLink aria-label="twitter" link="https://twitter.com/spacedriveapp">
							<Twitter className="h-6 w-6" />
						</FooterLink>
						<FooterLink aria-label="discord" link="https://discord.gg/gTaF2Z44f5">
							<Discord className="h-6 w-6" />
						</FooterLink>
						<FooterLink
							aria-label="instagram"
							link="https://instagram.com/spacedriveapp"
						>
							<Instagram className="h-6 w-6" />
						</FooterLink>
						<FooterLink aria-label="github" link="https://github.com/spacedriveapp">
							<Github className="h-6 w-6" />
						</FooterLink>
						<FooterLink
							aria-label="open collective"
							link="https://opencollective.com/spacedrive"
						>
							<Opencollective className="h-6 w-6" />
						</FooterLink>
						<FooterLink
							aria-label="twitch stream"
							link="https://twitch.tv/jamiepinelive"
						>
							<Twitch className="h-6 w-6" />
						</FooterLink>
					</div>
				</div>

				<div className="col-span-1 flex flex-col space-y-2">
					<h1 className="mb-1 text-xs font-bold uppercase ">About</h1>

					<FooterLink link="/team">Team</FooterLink>
					<FooterLink link="/docs/product/resources/faq">FAQ</FooterLink>
					<FooterLink link="/careers">Careers</FooterLink>
					<FooterLink link="/docs/changelog/beta/0.1.0">Changelog</FooterLink>
					<FooterLink link="/blog">Blog</FooterLink>
				</div>
				<div className="pointer-events-none col-span-1 flex flex-col space-y-2">
					<h1 className="mb-1 text-xs font-bold uppercase">Downloads</h1>
					<div className="col-span-1 flex flex-col space-y-2 opacity-50">
						<FooterLink link="#">macOS</FooterLink>
						<FooterLink link="#">Windows</FooterLink>
						<FooterLink link="#">Linux</FooterLink>
						<FooterLink link="#">Android</FooterLink>
						<FooterLink link="#">iOS</FooterLink>
					</div>
				</div>
				<div className="col-span-1 flex flex-col space-y-2">
					<h1 className="mb-1 text-xs font-bold uppercase ">Developers</h1>
					<FooterLink link="/docs/product/getting-started/introduction">
						Documentation
					</FooterLink>
					<FooterLink
						blank
						link="https://github.com/spacedriveapp/spacedrive/blob/main/CONTRIBUTING.md"
					>
						Contribute
					</FooterLink>
					<div className="pointer-events-none opacity-50">
						<FooterLink link="#">Extensions</FooterLink>
					</div>
					<div className="pointer-events-none opacity-50">
						<FooterLink link="#">Self Host</FooterLink>
					</div>
				</div>
				<div className="col-span-1 flex flex-col space-y-2">
					<h1 className="mb-1 text-xs font-bold uppercase ">Org</h1>
					<FooterLink blank link="https://opencollective.com/spacedrive">
						Open Collective
					</FooterLink>
					<FooterLink
						blank
						link="https://github.com/spacedriveapp/spacedrive/blob/main/LICENSE"
					>
						License
					</FooterLink>
					<div>
						<FooterLink link="/docs/company/legal/privacy">Privacy</FooterLink>
					</div>
					<div>
						<FooterLink link="/docs/company/legal/terms">Terms</FooterLink>
					</div>
				</div>
			</div>
			<div className="absolute top-0 flex h-1 w-full flex-row items-center justify-center opacity-100">
				<div className="h-[1px] w-1/2 bg-gradient-to-r from-transparent to-white/10"></div>
				<div className="h-[1px] w-1/2 bg-gradient-to-l from-transparent to-white/10"></div>
			</div>
		</footer>
	);
}
