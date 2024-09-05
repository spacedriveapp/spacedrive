import Image from 'next/image';
import React from 'react';
import assistantIcon from '~/assets/assistant_icon.webp';
import Particles from '~/components/particles';

export const Assistant = () => {
	return (
		<section className="relative w-full">
			{/* Background Image positioned absolutely */}
			<Image
				src="/images/new/comet_bg.svg"
				alt=""
				width={2000}
				height={1200}
				className="absolute bottom-[-300px] left-0 -z-10 overflow-visible"
			/>

			<div className="container relative z-10 mx-auto flex w-full flex-col flex-wrap items-start p-4">
				{/* Somewhere there is some mystery padding here. IDK, but magic I guess - @Rocky43007  */}

				<Image
					className="-ms-8 flex size-64 shrink-0 items-center justify-center"
					quality={100}
					src={assistantIcon}
					width={251}
					height={251}
					alt=""
				/>

				<h2 className="flex-1 self-start text-2xl font-bold leading-8 md:text-3xl md:leading-10">
					Assistant.{' '}
					<span className="bg-gradient-to-r from-[#D1DCFF] via-[#A8BEFF] via-30% to-[#E771FF] bg-clip-text font-semibold text-transparent">
						Mighty-powerful AI. No cloud needed.
					</span>
				</h2>
				<div className="absolute inset-0 -z-10 mx-auto mix-blend-overlay">
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
						<p className="text-center font-plex text-[14px] font-[500] leading-[125%] text-[#FFF]">
							COMING NEXT YEAR
						</p>
					</div>
				</div>
			</div>
		</section>
	);
};
