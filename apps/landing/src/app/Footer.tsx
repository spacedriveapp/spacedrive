'use client';

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
import companyLogoFull from '~/assets/company_full_logo.svg?url';

import { getLatestRelease } from './docs/changelog/data';
import { DownloadButton } from './Downloads/Button';
import { useCurrentPlatform } from './Downloads/Platform';

export function Footer() {
	const currentPlatform = useCurrentPlatform();

	return (
		<>
			{/* Download Button */}
			<div className="col-span-2 flex translate-y-3 flex-col items-center justify-center">
				<div className="translate-y-3.5">
					<DownloadButton
						name={currentPlatform?.name ?? 'macOS'}
						link={`https://spacedrive.com/api/releases/desktop/stable/${currentPlatform?.os}/x86_64`}
					/>
				</div>
			</div>
			<footer
				className="py-12 text-white"
				style={{
					backgroundImage: `url('data:image/svg+xml,%3Csvg xmlns%3D%22http%3A//www.w3.org/2000/svg%22 viewBox%3D%220 0 1920 420%22 fill%3D%22none%22%3E%3Cpath d%3D%22M0 49.5L-0.0924661 48.5043L-1 48.5886V49.5V419.998V420.998H0H1920H1921V419.998V49.5V48.5886L1920.09 48.5043L1920 49.5C1920.09 48.5043 1920.09 48.5041 1920.09 48.5037L1920.07 48.5021L1920 48.4957L1919.73 48.4704L1918.63 48.3702C1917.67 48.2817 1916.23 48.1506 1914.33 47.9795C1910.53 47.6373 1904.91 47.1354 1897.58 46.4967C1882.93 45.2193 1861.5 43.3945 1834.35 41.2048C1780.04 36.8253 1702.88 30.9861 1611.44 25.147C1428.55 13.4688 1188.53 1.78979 960 1.78968C731.47 1.78958 491.446 13.4685 308.561 25.1468C217.117 30.986 139.955 36.8252 85.654 41.2047C58.5032 43.3945 37.0674 45.2193 22.419 46.4967C15.0947 47.1354 9.46731 47.6373 5.67073 47.9795C3.77243 48.1506 2.33185 48.2817 1.36574 48.3702C0.882678 48.4144 0.518236 48.4479 0.274504 48.4704L-0.000299077 48.4957L-0.0693103 48.5021L-0.0866324 48.5037C-0.0905066 48.5041 -0.0924661 48.5043 0 49.5Z%22 fill%3D%22%23141419%22 stroke%3D%22url%28%23paint0_linear_1926_240%29%22 stroke-width%3D%222%22/%3E%3Cdefs%3E%3ClinearGradient id%3D%22paint0_linear_1926_240%22 x1%3D%220%22 y1%3D%22167.5%22 x2%3D%221920%22 y2%3D%22167.5%22 gradientUnits%3D%22userSpaceOnUse%22%3E%3Cstop offset%3D%220.319255%22 stop-color%3D%22%231E1E26%22 stop-opacity%3D%220.4%22/%3E%3Cstop offset%3D%220.495%22 stop-color%3D%22%233692DF%22/%3E%3Cstop offset%3D%220.661993%22 stop-color%3D%22%231E1E26%22 stop-opacity%3D%220.5%22/%3E%3C/linearGradient%3E%3C/defs%3E%3C/svg%3E')`,
					backgroundSize: 'cover',
					backgroundPosition: 'center'
				}}
			>
				<div className="stroke-[gba(30, 30, 38, 0.40)] container mx-auto grid min-h-64 w-full max-w-[100rem] flex-shrink-0 grid-cols-1 gap-8 fill-[#141419] stroke-[2px] px-8 pt-[5.388125rem] lg:grid-cols-6">
					<div className="col-span-2 flex flex-row items-center">
						<div>
							<Image
								src={companyLogoFull}
								alt="Spacedrive Technology Inc."
								width={262}
								height={73}
								className="mr-4" // Use 'mr-4' to add spacing between the image and the text
							/>
							<p className="ml-[4.575rem] mt-2 text-ink-faint">
								329 Railway St
								<br />
								Vancouver, BC V6A 1A4
							</p>
						</div>
					</div>

					{/* Product Links */}
					<div className="text-gray-400">
						<h2 className="mb-4 text-sm font-semibold text-gray-500">PRODUCT</h2>
						<ul>
							<li className="mb-2 hover:text-white">
								<a href="#">Explorer</a>
							</li>
							<li className="mb-2 hover:text-white">
								<a href="#">Teams</a>
							</li>
							<li className="mb-2 hover:text-white">
								<a href="#">Assistant</a>
							</li>
							<li className="mb-2 hover:text-white">
								<a href="#">Changelog</a>
							</li>
						</ul>
					</div>

					{/* Download Links */}
					<div className="text-gray-400">
						<h2 className="mb-4 text-sm font-semibold text-gray-500">DOWNLOADS</h2>
						<ul>
							<li className="mb-2 hover:text-white">
								<a href="https://spacedrive.com/api/releases/desktop/stable/darwin/aarch64">
									macOS
								</a>
							</li>
							<li className="mb-2 hover:text-white">
								<a href="https://spacedrive.com/api/releases/desktop/stable/darwin/x86_64">
									macOS - Intel
								</a>
							</li>
							<li className="mb-2 hover:text-white">
								<a href="https://spacedrive.com/api/releases/desktop/stable/windows/x86_64">
									Windows
								</a>
							</li>
							<li className="mb-2 hover:text-white">
								<a href="https://spacedrive.com/api/releases/desktop/stable/linux/x86_64">
									Linux
								</a>
							</li>
							<li className="pointer-events-none mb-2 text-gray-450">
								<a href="#">iOS</a>
							</li>
							<li className="pointer-events-none mb-2 text-gray-450">
								<a href="#">Android</a>
							</li>
						</ul>
					</div>

					{/* Developer Links */}
					<div className="text-gray-400">
						<h2 className="mb-4 text-sm font-semibold text-gray-500">DEVELOPERS</h2>
						<ul>
							<li className="mb-2 hover:text-white">
								<a href="#">Documentation</a>
							</li>
							<li className="mb-2 hover:text-white">
								<a href="#">Contribute</a>
							</li>
							<li className="mb-2 hover:text-white">
								<a href="#">Extensions</a>
							</li>
							<li className="mb-2 hover:text-white">
								<a href="#">Self Host</a>
							</li>
						</ul>
					</div>

					{/* Company Links */}
					<div className="text-gray-400">
						<h2 className="mb-4 text-sm font-semibold text-gray-500">COMPANY</h2>
						<ul>
							<li className="mb-2 hover:text-white">
								<a href="#">Open Collective</a>
							</li>
							<li className="mb-2 hover:text-white">
								<a href="#">License</a>
							</li>
							<li className="mb-2 hover:text-white">
								<a href="#">Privacy</a>
							</li>
							<li className="mb-2 hover:text-white">
								<a href="#">Terms</a>
							</li>
						</ul>
					</div>
				</div>
			</footer>
		</>
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
