'use client';

// Ensure this component runs on the client-side
import Image from 'next/image';
import React from 'react';
import Tilt from 'react-parallax-tilt';

interface HeroImageProps {
	src: string;
	alt: string;
	width: number;
	height: number;
}

export const HeroImage: React.FC<HeroImageProps> = ({ src, alt, width, height }) => {
	return (
		<Tilt tiltMaxAngleX={2} tiltMaxAngleY={2} transitionSpeed={5000} glareEnable={false}>
			<div className="relative m-auto mt-10 flex w-full max-w-7xl overflow-hidden rounded-[10px] transition-transform duration-700 ease-in-out md:mt-0">
				<div className="flex flex-col items-center justify-center">
					<div className="z-30 flex w-full justify-center backdrop-blur">
						<div className="relative h-auto w-full max-w-[1200px]">
							<div className="h-px w-full bg-gradient-to-r from-transparent via-[#008BFF]/40 to-transparent" />
							<div className="absolute inset-x-0 top-0 z-[110] size-full bg-gradient-to-b from-transparent from-30% to-[#0E0E12] to-80%" />
							<Image
								loading="eager"
								layout="responsive"
								width={width}
								quality={100}
								height={height}
								alt={alt}
								src={src}
							/>
						</div>
					</div>
				</div>
			</div>
		</Tilt>
	);
};
