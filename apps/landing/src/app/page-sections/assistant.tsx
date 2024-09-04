import Image from 'next/image';
import React from 'react';

import Particles from '../../particles';

export const Assistant = () => {
	return (
		<div className="relative w-full">
			{/* Background Image positioned absolutely */}
			<Image
				src="/images/new/comet_bg.svg"
				alt="Background"
				width={2000}
				height={1200}
				className="absolute bottom-[-300px] left-0 -z-10 overflow-visible"
			/>

			<div className="relative z-10 mx-auto flex w-full max-w-[1200px] flex-col flex-wrap items-start p-4">
				{/* Somewhere there is some mystery padding here. IDK, but magic I guess - @Rocky43007  */}

				<Image
					className="flex size-[251px] shrink-0 items-center justify-center p-[31.375px]"
					src="/images/new/assistant.svg"
					width={188.25}
					height={188.25}
					alt="Assistant"
				/>

				<h1 className="flex-1 self-start text-2xl font-semibold leading-8 md:text-3xl md:leading-10">
					Assistant.{' '}
					<span className="bg-gradient-to-r from-zinc-400 to-zinc-600 bg-clip-text text-transparent">
						Mighty-powerful AI. No cloud needed.
					</span>
				</h1>
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

				<h2 className="mt-[12px] text-lg text-ink-faint">Details to be revealed soon...</h2>

				<div className="mt-[32.73px]">
					<div className="inline-flex items-center justify-center gap-[10px] rounded-full border-2 border-[#FC79E7] bg-[rgba(43,43,59,0.50)] px-[11px] py-[10px]">
						<p className="font-plex text-center text-[14px] font-[500] leading-[125%] text-[#FFF]">
							COMING NEXT YEAR
						</p>
					</div>
				</div>
			</div>
		</div>
	);
};
