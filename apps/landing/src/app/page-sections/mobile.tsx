'use client';

import { GooglePlayLogo } from '@phosphor-icons/react';
import { Apple } from '@sd/assets/svgs/brands';
import Image from 'next/image';
import { useEffect, useState } from 'react';
import { useParallax } from 'react-scroll-parallax';
import { tw } from '@sd/ui';

import { CtaSecondaryButton } from '../../components/cta-secondary-button';

const Mobile = () => {
	const [screenSize, setScreenSize] = useState('sm');

	const parallax1 = useParallax<HTMLDivElement>({ speed: 5, translateY: [0, -15] });
	const parallax2 = useParallax<HTMLDivElement>({ speed: 8, translateY: [0, -25] });
	const parallax3 = useParallax<HTMLDivElement>({ speed: 10, translateY: [0, -40] });
	const parallax4 = useParallax<HTMLDivElement>({ speed: 8, translateY: [0, -25] });
	const parallax5 = useParallax<HTMLDivElement>({ speed: 5, translateY: [0, -15] });

	const parallaxRefs = [parallax1, parallax2, parallax3, parallax4, parallax5];

	useEffect(() => {
		const handleResize = () => {
			if (window.innerWidth >= 1024) setScreenSize('lg');
			else if (window.innerWidth >= 768) setScreenSize('md');
			else setScreenSize('sm');
		};

		handleResize();
		window.addEventListener('resize', handleResize);
		return () => window.removeEventListener('resize', handleResize);
	}, []);

	const getSpacing = () => {
		switch (screenSize) {
			case 'lg':
				return { x: 160, y: 30, width: 250, containerWidth: 1200 };
			case 'md':
				return { x: 130, y: 25, width: 200, containerWidth: 960 };
			default:
				return { x: 80, y: 15, width: 150, containerWidth: 720 };
		}
	};

	const spacing = getSpacing();
	const totalWidth = spacing.x * 4 + spacing.width;

	return (
		<div className="container mx-auto -mt-24 flex flex-col flex-wrap items-center gap-10 px-4">
			<div className="relative w-full">
				<div
					className="relative mx-auto overflow-hidden"
					style={{
						width: '100%',
						maxWidth: `${spacing.containerWidth}px`,
						height: 'min(55vw, 570px)'
					}}
				>
					<div className="absolute inset-0 z-10 bg-gradient-to-t from-[#090909] to-transparent" />
					<div
						className="absolute left-1/2 h-full w-full"
						style={{
							transform: 'translateX(-50%)',
							maxWidth: `${totalWidth}px`,
							minWidth: `${totalWidth}px`
						}}
					>
						{[1, 2, 3, 4, 5].map((num, i) => (
							<div
								key={num}
								ref={parallaxRefs[i].ref}
								className="absolute"
								style={{
									left: `${i * spacing.x}px`,
									top: `${Math.abs(2 - i) * spacing.y + 200}px`,
									width: `${spacing.width}px`,
									zIndex: 5 - Math.abs(2 - i)
								}}
							>
								<Image
									loading="eager"
									quality={100}
									width={250}
									height={700}
									className="object-contain transition-all duration-300"
									style={{
										transform: `rotate(${(i - 2) * 10}deg)`,
										width: '100%'
									}}
									src={`/images/app/mobile${num}.png`}
									alt={`Mobile Screenshot ${num}`}
								/>
							</div>
						))}
					</div>
				</div>
			</div>
			<div className="relative z-20 -mt-32 flex flex-col gap-1 md:-mt-24">
				<h1 className="flex flex-col self-center text-center text-2xl font-semibold md:flex-row md:text-3xl">
					Cross platform.&nbsp;
					<span className="bg-gradient-to-r from-zinc-400 to-zinc-600 bg-clip-text text-transparent">
						<br className="hidden lg:visible" />
						Available on iOS and Android
					</span>
				</h1>
				<p className="w-full max-w-[600px] text-center text-ink-faint">
					Access your files from anywhere, on any device, with the Spacedrive app.
				</p>
				<div className="mx-auto mt-4 flex flex-row flex-wrap justify-center gap-4">
					{/* <CtaSecondaryButton icon={<GooglePlayLogo />}>
						Open Play Store
					</CtaSecondaryButton>
					<CtaSecondaryButton icon={<Apple className="size-4" />}>
						Open App Store
					</CtaSecondaryButton> */}
					<CtaSecondaryButton
						icon={<></>}
						disabled={true}
					>
						Coming Soon
					</CtaSecondaryButton>
				</div>
			</div>
		</div>
	);
};

export { Mobile };
