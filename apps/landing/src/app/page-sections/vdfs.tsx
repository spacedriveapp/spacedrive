'use client';

import { useParallax } from 'react-scroll-parallax';
import Particles from '~/components/particles';

import { Icon } from '../Icon';

export function Vdfs() {
	const parallax = useParallax<HTMLDivElement>({
		speed: 5
	});

	return (
		<div className="flex flex-col items-center justify-center px-5 py-32 text-center">
			<p className="fade-in-heading mb-2 text-sm text-gray-400">POWERED BY THE</p>
			<h2 className="animation-delay-2 fade-in-heading bg-gradient-to-r from-purple-400 to-pink-400 bg-clip-text text-xl font-bold text-transparent">
				VIRTUAL DISTRIBUTED FILESYSTEM
			</h2>
			<div className="relative">
				<h1 className="animation-delay-3 fade-in-heading -mt-8 bg-gradient-to-b from-[#FF9EF1] to-[#7129FF] bg-clip-text text-[198px] font-bold leading-[120%] tracking-tighter text-transparent">
					VDFS
				</h1>
				<div className="pointer-events-none absolute inset-0 mt-[270px] flex items-center justify-center">
					<div ref={parallax.ref}>
						<Icon
							name="Ball"
							size={400}
							className="fade-in-heading animation-delay-4 relative z-10"
						/>
					</div>
					<div className="absolute inset-0 z-20">
						<div className="mt-[-210px] h-[400px] w-full bg-gradient-to-t from-[#090909] via-[#090909]/70 to-transparent" />
					</div>
				</div>
			</div>
			<div className="absolute inset-x-0 -z-10 mix-blend-overlay">
				<Particles
					quantity={250}
					ease={100}
					staticity={200}
					color={'#ADAFCD'}
					className="opacity-80"
					refresh
					vy={-0.005}
					vx={-0.0005}
				/>
			</div>
			<div className="z-30">
				<h1 className="mb-4 mt-[200px] text-4xl font-bold">
					Your Files. Every Device. Instantly.
				</h1>
				<p className="max-w-2xl text-lg text-gray-400">
					Built in Rust for unmatched speed and memory safety, the VDFS unites your devices
					into a single, seamless file management experience.
				</p>
			</div>
		</div>
	);
}
