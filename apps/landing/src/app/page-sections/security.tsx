'use client';

import Image from 'next/image';
import { useEffect, useRef, useState } from 'react';

function BinaryBackground() {
	const [digits, setDigits] = useState<string[][]>([]);
	const layoutRef = useRef<{ length: number; offset: number }[]>([]);

	useEffect(() => {
		// Generate layout structure once
		if (layoutRef.current.length === 0) {
			layoutRef.current = Array.from({ length: 8 }, () => ({
				length: Math.floor(Math.random() * 15) + 25, // 35-45 digits per row
				offset: Math.random() * 40 // Fixed offset for each row
			}));
		}

		const generateDigits = () => {
			const newDigits = layoutRef.current.map((row) =>
				Array.from({ length: row.length }, () => (Math.random() > 0.5 ? '1' : '0'))
			);
			setDigits(newDigits);
		};

		generateDigits();
		const interval = setInterval(generateDigits, 100);
		return () => clearInterval(interval);
	}, []);

	return (
		<div className="absolute inset-0 flex h-[500px] w-full items-center justify-center overflow-hidden">
			<div
				className="flex w-full flex-col gap-4 px-20 opacity-20"
				style={{
					mask: 'linear-gradient(90deg, transparent 0%, black 25%, black 75%, transparent 100%)',
					WebkitMask:
						'linear-gradient(90deg, transparent 0%, black 25%, black 75%, transparent 100%)'
				}}
			>
				{digits.map((row, rowIndex) => (
					<div
						key={rowIndex}
						className="flex w-full justify-between"
						style={{
							paddingLeft: `${layoutRef.current[rowIndex]?.offset}px`,
							transform: `translateX(${Math.sin(rowIndex) * 20}px)`
						}}
					>
						{row.map((digit, i) => (
							<span key={i} className="font-mono text-sm">
								{digit}
							</span>
						))}
					</div>
				))}
			</div>
		</div>
	);
}

export function Security() {
	return (
		<div className="container mx-auto flex w-full flex-col justify-center gap-2 p-4">
			<div className="relative flex items-center justify-center">
				<BinaryBackground />
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
						className="origin-center animate-[spin_10s_linear_infinite]"
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
				<h1 className="mb-4 text-center text-4xl font-bold">Designed For Privacy</h1>
				<p className="max-w-2xl text-center text-lg text-gray-400"></p>
			</div>
		</div>
	);
}
