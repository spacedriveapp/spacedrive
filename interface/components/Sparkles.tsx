'use client';

import { useState } from 'react';
import { usePrefersReducedMotion, useRandomInterval } from '~/hooks';

const DEFAULT_COLOR = '#FFC700';

const random = (min: number, max: number) => Math.floor(Math.random() * (max - min)) + min;

const range = (start: number, end?: number, step = 1) => {
	const output = [];
	if (typeof end === 'undefined') {
		end = start;
		start = 0;
	}
	for (let i = start; i < end; i += step) {
		output.push(i);
	}
	return output;
};

const generateSparkle = (color: string) => {
	const sparkle = {
		id: String(random(10000, 99999)),
		createdAt: Date.now(),
		color,
		size: random(10, 20),
		style: {
			top: random(0, 100) + '%',
			left: random(0, 100) + '%'
		}
	};
	return sparkle;
};

type SparklesProps = {
	color?: string;
	children: React.ReactNode;
};

// million-ignore
const Sparkles = ({ color = DEFAULT_COLOR, children }: SparklesProps) => {
	const [sparkles, setSparkles] = useState(() => {
		return range(3).map(() => generateSparkle(color));
	});
	const prefersReducedMotion = usePrefersReducedMotion();

	useRandomInterval(
		() => {
			const sparkle = generateSparkle(color);
			const now = Date.now();
			const nextSparkles = sparkles.filter((sp) => {
				const delta = now - sp.createdAt;
				return delta < 750;
			});
			nextSparkles.push(sparkle);
			setSparkles(nextSparkles);
		},
		prefersReducedMotion ? null : 100,
		prefersReducedMotion ? null : 1000
	);

	return (
		<span className="relative inline-block">
			{sparkles.map((sparkle) => (
				<span
					key={sparkle.id}
					className="z-10"
					style={{
						position: 'absolute',
						display: 'block',
						animation: prefersReducedMotion ? 'none' : 'comeInOut 700ms forwards',
						...sparkle.style
					}}
				>
					<svg
						width={sparkle.size}
						height={sparkle.size}
						viewBox="0 0 68 68"
						fill="none"
						style={{
							display: 'block',
							animation: prefersReducedMotion ? 'none' : 'spin 1000ms linear'
						}}
					>
						<path
							d="M26.5 25.5C19.0043 33.3697 0 34 0 34C0 34 19.1013 35.3684 26.5 43.5C33.234 50.901 34 68 34 68C34 68 36.9884 50.7065 44.5 43.5C51.6431 36.647 68 34 68 34C68 34 51.6947 32.0939 44.5 25.5C36.5605 18.2235 34 0 34 0C34 0 33.6591 17.9837 26.5 25.5Z"
							fill={sparkle.color}
						/>
					</svg>
				</span>
			))}
			<strong style={{ position: 'relative', zIndex: 1, fontWeight: 'bold' }}>
				{children}
			</strong>
		</span>
	);
};

export default Sparkles;
