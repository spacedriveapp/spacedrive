'use client';

import Image from 'next/image';
import { useEffect, useRef, useState } from 'react';

function BinaryBackground({ isInView }: { isInView: boolean }) {
	const [digits, setDigits] = useState<string[][]>([]);
	const [isVisible, setIsVisible] = useState(false);
	const layoutRef = useRef<{ length: number; offset: number }[]>([]);
	const animationStartedRef = useRef(false);

	useEffect(() => {
		if (isInView && !animationStartedRef.current) {
			animationStartedRef.current = true;

			if (layoutRef.current.length === 0) {
				layoutRef.current = Array.from({ length: 8 }, () => ({
					length: Math.floor(Math.random() * 15) + 25,
					offset: Math.random() * 40
				}));
			}

			// Wait for handle snap to almost complete (1.85s is 92.5% of 2s animation)
			setTimeout(() => {
				setIsVisible(true);
			}, 1250);

			const generateDigits = () => {
				const newDigits = layoutRef.current.map((row) =>
					Array.from({ length: row.length }, () => (Math.random() > 0.5 ? '1' : '0'))
				);
				setDigits(newDigits);
			};

			generateDigits();
			const interval = setInterval(generateDigits, 100);
			return () => clearInterval(interval);
		} else {
			setIsVisible(false);
			animationStartedRef.current = false;
		}
	}, [isInView]);

	if (!isVisible) return null;

	// Calculate center points
	const centerRow = Math.floor(layoutRef.current.length / 2);
	const maxDistance = Math.sqrt(
		Math.pow(layoutRef.current.length, 2) +
			Math.pow(Math.max(...layoutRef.current.map((r) => r.length)), 2)
	);

	return (
		<div className="absolute inset-0 flex h-[500px] w-full items-center justify-center overflow-hidden">
			<div
				className="flex w-full flex-col gap-4 px-20"
				style={{
					mask: 'linear-gradient(90deg, transparent 0%, black 25%, black 75%, transparent 100%)',
					WebkitMask:
						'linear-gradient(90deg, transparent 0%, black 25%, black 75%, transparent 100%)'
				}}
			>
				{digits.map((row, rowIndex) => {
					const centerCol = Math.floor(row.length / 2);

					return (
						<div
							key={rowIndex}
							className="flex w-full justify-between"
							style={{
								paddingLeft: `${layoutRef.current[rowIndex]?.offset}px`,
								transform: `translateX(${Math.sin(rowIndex) * 20}px)`
							}}
						>
							{row.map((digit, colIndex) => {
								// Calculate distance from center
								const distanceFromCenter = Math.sqrt(
									Math.pow(rowIndex - centerRow, 2) +
										Math.pow(colIndex - centerCol, 2)
								);

								// Normalize distance to 0-1 range and use it for delay
								const normalizedDistance = distanceFromCenter / maxDistance;
								const delay = normalizedDistance * 1.5; // Increased to 1.5s max delay

								return (
									<span
										key={colIndex}
										className="font-mono animate-digit-reveal text-sm"
										style={{
											animationDelay: `${delay}s`,
											animationFillMode: 'both'
										}}
									>
										{digit}
									</span>
								);
							})}
						</div>
					);
				})}
			</div>
		</div>
	);
}

export function Security() {
	const [isInView, setIsInView] = useState(false);
	const ref = useRef<HTMLDivElement>(null);

	useEffect(() => {
		const observer = new IntersectionObserver(
			([entry]) => {
				setIsInView(entry.isIntersecting);
			},
			{
				threshold: 0.5 // Trigger when 50% of the component is visible
			}
		);

		if (ref.current) {
			observer.observe(ref.current);
		}

		return () => {
			observer.disconnect();
		};
	}, []);

	return (
		<div className="container mx-auto flex w-full flex-col justify-center gap-2 p-4">
			<div ref={ref} className="relative flex items-center justify-center">
				<BinaryBackground isInView={isInView} />
				<Image
					quality={100}
					src="/images/vault-base.png"
					width={500}
					height={500}
					alt="Spacedrive vault"
					className="relative"
				/>
				<div className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2">
					<Image
						quality={100}
						src="/images/vault-handle.png"
						width={160}
						height={160}
						alt="Spacedrive vault"
						className={
							isInView ? 'animate-handle-rotate origin-center' : 'origin-center'
						}
					/>
				</div>
				<Image
					quality={100}
					src="/images/vault-light.png"
					width={500}
					height={500}
					alt="Spacedrive vault"
					className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 mix-blend-color-dodge"
				/>
			</div>

			<div className="z-30 mt-20 flex w-full flex-col items-center">
				<h1 className="mb-4 text-center text-4xl font-bold">Designed for privacy</h1>
				<p className="max-w-2xl text-center text-lg text-gray-400">
					Spacedrive is designed from the ground up to be secure and ensure your data is
					safe, with client-side, end-to-end encryption.
				</p>
			</div>
		</div>
	);
}
