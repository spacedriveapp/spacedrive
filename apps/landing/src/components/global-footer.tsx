'use client';

import Image from 'next/image';
import companyLogoFull from '~/assets/company_full_logo.svg?url';
import { CtaPrimaryButton } from '~/components/cta-primary-button';
import { useCurrentPlatform } from '~/utils/current-platform';

import Particles from './particles';

export function GlobalFooter() {
	const currentPlatform = useCurrentPlatform();

	return (
		<div className="relative">
			{/* Download Button */}
			<div className="relative z-20 col-span-2 mt-20 flex translate-y-2 flex-col items-center justify-center">
				<div className="relative translate-y-3.5">
					<CtaPrimaryButton platform={currentPlatform} />
				</div>
			</div>
			<div className="absolute inset-x-0 top-[-40px] mx-auto size-[200px] md:top-[-70px] md:size-[500px]">
				<Particles
					quantity={50}
					ease={80}
					staticity={100}
					color={'#58B3FF'}
					refresh
					vy={-0.2}
					vx={-0.05}
				/>
			</div>
			<div className="animation-delay-2 absolute-horizontal-center top-[-40px] h-[250px] w-[100px] rounded-full bg-gradient-to-b from-transparent to-[#328FDD]/30 blur-[70px] fade-in md:w-[550px]" />
			<footer
				className="relative z-10 overflow-hidden bg-center pt-12 text-gray-200 lg:bg-top"
				style={{
					backgroundImage: `url('data:image/svg+xml,%3Csvg xmlns%3D%22http%3A//www.w3.org/2000/svg%22 viewBox%3D%220 0 1920 420%22 fill%3D%22none%22%3E%3Cpath d%3D%22M0 49.5L-0.0924661 48.5043L-1 48.5886V49.5V419.998V420.998H0H1920H1921V419.998V49.5V48.5886L1920.09 48.5043L1920 49.5C1920.09 48.5043 1920.09 48.5041 1920.09 48.5037L1920.07 48.5021L1920 48.4957L1919.73 48.4704L1918.63 48.3702C1917.67 48.2817 1916.23 48.1506 1914.33 47.9795C1910.53 47.6373 1904.91 47.1354 1897.58 46.4967C1882.93 45.2193 1861.5 43.3945 1834.35 41.2048C1780.04 36.8253 1702.88 30.9861 1611.44 25.147C1428.55 13.4688 1188.53 1.78979 960 1.78968C731.47 1.78958 491.446 13.4685 308.561 25.1468C217.117 30.986 139.955 36.8252 85.654 41.2047C58.5032 43.3945 37.0674 45.2193 22.419 46.4967C15.0947 47.1354 9.46731 47.6373 5.67073 47.9795C3.77243 48.1506 2.33185 48.2817 1.36574 48.3702C0.882678 48.4144 0.518236 48.4479 0.274504 48.4704L-0.000299077 48.4957L-0.0693103 48.5021L-0.0866324 48.5037C-0.0905066 48.5041 -0.0924661 48.5043 0 49.5Z%22 fill%3D%22%23141419%22 stroke%3D%22url%28%23paint0_linear_1926_240%29%22 stroke-width%3D%222%22/%3E%3Cdefs%3E%3ClinearGradient id%3D%22paint0_linear_1926_240%22 x1%3D%220%22 y1%3D%22167.5%22 x2%3D%221920%22 y2%3D%22167.5%22 gradientUnits%3D%22userSpaceOnUse%22%3E%3Cstop offset%3D%220.319255%22 stop-color%3D%22%231E1E26%22 stop-opacity%3D%220.4%22/%3E%3Cstop offset%3D%220.495%22 stop-color%3D%22%233692DF%22/%3E%3Cstop offset%3D%220.661993%22 stop-color%3D%22%231E1E26%22 stop-opacity%3D%220.5%22/%3E%3C/linearGradient%3E%3C/defs%3E%3C/svg%3E')`,
					backgroundSize: 'cover'
				}}
			>
				<div className="container mx-auto grid h-fit w-full shrink-0 grid-cols-1 grid-rows-3 gap-x-8 gap-y-16 fill-[#141419] px-8 py-20 lg:grid-cols-6 lg:grid-rows-1 lg:gap-y-8">
					<div className="order-10 col-span-2 -ml-5 flex flex-col pt-5 font-sans lg:order-[unset]">
						<Image
							src={companyLogoFull}
							alt="Spacedrive Technology Inc."
							width={262}
							height={73}
							className="mr-4"
						/>
						<p className="mb-2 ml-[4.675rem] mt-4 text-xs tracking-[0.01em] text-ink-faint">
							<b className="text-white">Spacedrive Technology Inc.</b>
							<br />
							651 N Broad St, Suite 201
							<br />
							Middletown, DE 19709
							<br />
							United States
						</p>

						<p className="ml-[4.675rem] mt-0 text-xs tracking-[0.01em] text-ink-faint">
							<b className="text-white">Spacedrive Studio</b>
							<br />
							402, 329 Railway St
							<br />
							Vancouver, BC V6A 1A4
							<br />
							Canada
						</p>
					</div>

					{/* Product Links */}
					<div>
						<h2 className="mb-4 font-semibold uppercase leading-[1.15] tracking-[0.08em] text-ink-faint/80">
							Product
						</h2>
						<ul className="flex flex-col gap-2.5 tracking-[0.04em]">
							<li>
								<a className="transition-colors hover:text-white" href="#">
									Explorer
								</a>
							</li>
							<li className="w-fit cursor-not-allowed">
								<a className="transition-colors hover:text-white" href="#">
									Cloud
								</a>
							</li>
							<li className="w-fit cursor-not-allowed">
								<a
									className="pointer-events-none cursor-not-allowed text-gray-450"
									href="#"
								>
									Teams
								</a>
							</li>
							<li className="w-fit cursor-not-allowed">
								<a
									className="pointer-events-none cursor-not-allowed text-gray-450"
									href="#"
								>
									Organizer
								</a>
							</li>
							<li>
								<a className="transition-colors hover:text-white" href="#">
									Changelog
								</a>
							</li>
						</ul>
					</div>

					{/* Download Links */}
					<div className="col-span-1">
						<h2 className="mb-4 font-semibold uppercase leading-[1.15] tracking-[0.08em] text-ink-faint/80">
							Downloads
						</h2>
						<ul className="flex flex-col gap-2.5 tracking-[0.04em]">
							<li>
								<a
									className="group relative transition-colors hover:text-white"
									href="https://spacedrive.com/api/releases/desktop/stable/darwin/aarch64"
								>
									macOS
									<span className="ml-1.5 rounded bg-primary-500 px-1.5 py-0.5 text-xs text-white opacity-0 transition-opacity group-hover:opacity-100">
										alpha
									</span>
								</a>
							</li>
							<li>
								<a
									className="group relative transition-colors hover:text-white"
									href="https://spacedrive.com/api/releases/desktop/stable/darwin/x86_64"
								>
									macOS - Intel
									<span className="ml-1.5 rounded bg-primary-500 px-1.5 py-0.5 text-xs text-white opacity-0 transition-opacity group-hover:opacity-100">
										alpha
									</span>
								</a>
							</li>
							<li>
								<a
									className="group relative transition-colors hover:text-white"
									href="https://spacedrive.com/api/releases/desktop/stable/windows/x86_64"
								>
									Windows
									<span className="ml-1.5 rounded bg-primary-500 px-1.5 py-0.5 text-xs text-white opacity-0 transition-opacity group-hover:opacity-100">
										alpha
									</span>
								</a>
							</li>
							<li>
								<a
									className="group relative transition-colors hover:text-white"
									href="https://spacedrive.com/api/releases/desktop/stable/linux/x86_64"
								>
									Linux
									<span className="ml-1.5 rounded bg-primary-500 px-1.5 py-0.5 text-xs text-white opacity-0 transition-opacity group-hover:opacity-100">
										alpha
									</span>
								</a>
							</li>
							<li className="w-fit">
								<a href="#" className="group relative">
									iOS
									<span className="ml-1.5 rounded bg-amber-500 px-1.5 py-0.5 text-xs text-white opacity-0 transition-opacity group-hover:opacity-100">
										testing
									</span>
								</a>
							</li>
							<li className="w-fit">
								<a href="#" className="group relative">
									Android
									<span className="ml-1.5 rounded bg-amber-500 px-1.5 py-0.5 text-xs text-white opacity-0 transition-opacity group-hover:opacity-100">
										testing
									</span>
								</a>
							</li>
							<li className="w-fit">
								<a href="#" className="group relative">
									iPadOS
									<span className="ml-1.5 rounded bg-amber-500 px-1.5 py-0.5 text-xs text-white opacity-0 transition-opacity group-hover:opacity-100">
										testing
									</span>
								</a>
							</li>
							<li className="w-fit cursor-not-allowed">
								<a
									className="pointer-events-none cursor-not-allowed text-gray-450"
									href="#"
								>
									visionOS
								</a>
							</li>
							<li className="w-fit cursor-not-allowed">
								<a
									className="pointer-events-none cursor-not-allowed text-gray-450"
									href="#"
								>
									watchOS
								</a>
							</li>
							<li className="w-fit">
								<a
									className="group transition-colors hover:text-white"
									href="/docs/product/getting-started/setup#docker"
								>
									Docker
									<span className="ml-1.5 rounded bg-primary-500 px-1.5 py-0.5 text-xs text-white opacity-0 transition-opacity group-hover:opacity-100">
										alpha
									</span>
								</a>
							</li>
						</ul>
					</div>

					{/* Developer Links */}
					<div>
						<h2 className="mb-4 font-semibold uppercase leading-[1.15] tracking-[0.08em] text-ink-faint/80">
							Developers
						</h2>
						<ul className="flex flex-col gap-2.5 tracking-[0.04em]">
							<li>
								<a
									className="transition-colors hover:text-white"
									href="/docs/product/getting-started/introduction"
								>
									Documentation
								</a>
							</li>
							<li>
								<a
									className="transition-colors hover:text-white"
									href="https://github.com/spacedriveapp/spacedrive"
								>
									Contribute
								</a>
							</li>
							<li className="w-fit cursor-not-allowed">
								<a
									className="pointer-events-none cursor-not-allowed text-gray-450"
									href="#"
								>
									Extensions
								</a>
							</li>
							<li className="w-fit cursor-not-allowed">
								<a
									className="pointer-events-none cursor-not-allowed text-gray-450"
									href="#"
								>
									Self Host
								</a>
							</li>
						</ul>
					</div>

					{/* Company Links */}
					<div>
						<h2 className="mb-4 font-semibold uppercase leading-[1.15] tracking-[0.08em] text-ink-faint/80">
							Company
						</h2>
						<ul className="flex flex-col gap-2.5 tracking-[0.04em]">
							<li>
								<a className="transition-colors hover:text-white" href="/team">
									Team
								</a>
							</li>
							<li>
								<a
									className="transition-colors hover:text-white"
									href="https://opencollective.com/spacedrive"
								>
									Open Collective
								</a>
							</li>
							<li>
								<a className="transition-colors hover:text-white" href="#">
									License
								</a>
							</li>
							<li>
								<a
									className="transition-colors hover:text-white"
									href="/docs/company/legal/privacy"
								>
									Privacy
								</a>
							</li>
							<li>
								<a
									className="transition-colors hover:text-white"
									href="/docs/company/legal/terms"
								>
									Terms
								</a>
							</li>
						</ul>
					</div>
				</div>
			</footer>
		</div>
	);
}
