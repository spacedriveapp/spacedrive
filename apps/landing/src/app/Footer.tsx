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

import { getLatestRelease } from './docs/changelog/data';
import Logo from './logo.png';

export async function Footer() {
	const latestRelease = await getLatestRelease();

	return (
		<footer id="footer" className="relative z-50 w-screen overflow-hidden pt-3 backdrop-blur">
			<Image
				alt="footer gradient"
				className="absolute bottom-0 left-0 z-[-1]"
				quality={100}
				width={0}
				height={0}
				src="/images/misc/footer-gradient.webp"
				style={{ width: '100%', height: '400px' }}
				sizes="100vw"
			/>
			<div className="m-auto grid min-h-64 max-w-[100rem] grid-cols-2 gap-6 p-8 pb-20 pt-10 text-white sm:grid-cols-2 lg:grid-cols-6">
				<div className="col-span-2">
					<Image alt="Spacedrive logo" src={Logo} className="mb-5 size-10" />

					<h1 className="mb-1 text-xl font-bold">Spacedrive</h1>
					<p className="text-sm text-gray-350 opacity-50">
						&copy; Copyright {new Date().getFullYear()} Spacedrive Technology Inc.
					</p>
					<div className="mb-10 mt-12 flex flex-row space-x-3">
						<FooterLink link="https://x.com/spacedriveapp">
							<Twitter className="size-6" />
						</FooterLink>
						<FooterLink aria-label="discord" link="https://discord.gg/gTaF2Z44f5">
							<Discord className="size-6" />
						</FooterLink>
						<FooterLink
							aria-label="instagram"
							link="https://instagram.com/spacedriveapp"
						>
							<Instagram className="size-6" />
						</FooterLink>
						<FooterLink aria-label="github" link="https://github.com/spacedriveapp">
							<Github className="size-6" />
						</FooterLink>
						<FooterLink
							aria-label="open collective"
							link="https://opencollective.com/spacedrive"
						>
							<Opencollective className="size-6" />
						</FooterLink>
						<FooterLink
							aria-label="twitch stream"
							link="https://twitch.tv/jamiepinelive"
						>
							<Twitch className="size-6" />
						</FooterLink>
					</div>
				</div>

				<div className="col-span-1 flex flex-col space-y-2">
					<h1 className="mb-1 text-xs font-bold uppercase ">About</h1>

					<FooterLink link="/team">Team</FooterLink>
					<FooterLink link="/docs/product/resources/faq">FAQ</FooterLink>
					<FooterLink link="/careers">Careers</FooterLink>
					{latestRelease && (
						<FooterLink
							link={`/docs/changelog/${latestRelease.category}/${latestRelease.tag}`}
						>
							Changelog
						</FooterLink>
					)}
					<FooterLink link="/blog">Blog</FooterLink>
				</div>
				<div className="col-span-1 flex flex-col space-y-2">
					<h1 className="mb-1 text-xs font-bold uppercase">Downloads</h1>
					<div className="col-span-1 flex flex-col space-y-2">
						<FooterLink link="https://spacedrive.com/api/releases/desktop/stable/darwin/aarch64">
							macOS
						</FooterLink>
						<FooterLink link="https://spacedrive.com/api/releases/desktop/stable/darwin/x86_64">
							macOS Intel
						</FooterLink>
						<FooterLink link="https://spacedrive.com/api/releases/desktop/stable/windows/x86_64">
							Windows
						</FooterLink>
						<FooterLink link="https://spacedrive.com/api/releases/desktop/stable/linux/x86_64">
							Linux
						</FooterLink>
					</div>
					<div className="pointer-events-none col-span-1 flex flex-col space-y-2 opacity-50">
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
				<div className="h-px w-1/2 bg-gradient-to-r from-transparent to-white/10"></div>
				<div className="h-px w-1/2 bg-gradient-to-l from-transparent to-white/10"></div>
			</div>
		</footer>
	);
}

function FooterLink({
	blank,
	link,
	...props
}: PropsWithChildren<{ link: string; blank?: boolean }>) {
	return (
		<Link
			href={link}
			target={blank ? '_blank' : ''}
			className="text-gray-300 duration-300 hover:text-white hover:opacity-50"
			rel="noreferrer"
			{...props}
		>
			{props.children}
		</Link>
	);
}
