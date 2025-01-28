import Image from 'next/image';
import React from 'react';
import assistantIcon from '~/assets/assistant_icon.webp';
import Particles from '~/components/particles';

export const Assistant = () => {
	return (
		<section className="relative mb-32 w-full md:mb-0">
			{/* Background Image positioned absolutely */}
			<Image
				src="/images/new/comet_bg.svg"
				alt=""
				width={2000}
				height={1200}
				className="absolute left-0 top-0 -z-10 overflow-visible md:top-[-200px]"
			/>

			<div className="container relative z-10 mx-auto flex w-full flex-col flex-wrap items-start p-4">
				<h2 className="flex flex-wrap items-center gap-2 text-2xl font-bold leading-8 md:flex-1 md:text-3xl md:leading-10">
					<Image
						className="flex size-12 shrink-0 justify-center"
						quality={100}
						src={assistantIcon}
						width={251}
						height={251}
						alt=""
					/>
					Assistant.{' '}
					<span className="bg-gradient-to-r from-[#D1DCFF] via-[#A8BEFF] via-30% to-[#E771FF] bg-clip-text font-semibold text-transparent">
						Mighty-powerful AI. No cloud needed.
					</span>
				</h2>
				<div className="absolute inset-x-0 -z-10 mix-blend-overlay">
					<Particles
						quantity={50}
						ease={100}
						staticity={200}
						color={'#ADAFCD'}
						className="opacity-80"
						refresh
						vy={-0.005}
						vx={-0.0005}
					/>
				</div>

				<p className="mt-[12px] text-lg tracking-[0.01em] text-ink drop-shadow-md">
					Details to be revealed soon...
				</p>

				<div className="mt-[32.73px]">
					<div className="inline-flex items-center justify-center gap-[10px] rounded-full border-2 border-[#FC79E7] bg-[rgba(43,43,59,0.50)] px-[11px] py-[10px]">
						<p className="text-center text-[14px] font-[500] leading-[125%] text-[#FFF]">
							COMING NEXT YEAR
						</p>
					</div>
				</div>
			</div>
		</section>
	);
};
