import React, { useState } from 'react';
import { Tooltip } from '@sd/ui';
import { useIsDark } from '~/hooks';

const BARWIDTH = 710;

const lightenColor = (color: string, percent: number) => {
	const num = parseInt(color.replace('#', ''), 16);
	const amt = Math.round(2.55 * percent);
	const R = (num >> 16) + amt;
	const G = ((num >> 8) & 0x00ff) + amt;
	const B = (num & 0x0000ff) + amt;
	return `#${(
		0x1000000 +
		(R < 255 ? (R < 1 ? 0 : R) : 255) * 0x10000 +
		(G < 255 ? (G < 1 ? 0 : G) : 255) * 0x100 +
		(B < 255 ? (B < 1 ? 0 : B) : 255)
	)
		.toString(16)
		.slice(1)
		.toUpperCase()}`;
};

interface Section {
	name: string;
	value: bigint;
	color: string;
	tooltip: string;
}

interface StorageBarProps {
	sections: Section[];
}

const StorageBar: React.FC<StorageBarProps> = ({ sections }) => {
	const isDark = useIsDark();
	const [hoveredSectionIndex, setHoveredSectionIndex] = useState<number | null>(null);

	const totalSpace = sections.reduce((acc, section) => acc + section.value, 0n);

	const getPercentage = (value: bigint) => {
		if (value === 0n) return '0px';
		console.log(value);
		const percentage = Number((value * 100n) / totalSpace) / 100;
		const pixvalue = BARWIDTH * percentage;
		return `${pixvalue.toFixed(2)}px`;
	};

	return (
		<div className="w-auto p-3">
			<div className="relative mt-1 flex h-6 overflow-hidden rounded">
				{sections.map((section, index) => {
					const isHovered = hoveredSectionIndex === index;
					const isLast = index === sections.length - 1;

					return (
						<Tooltip key={index} label={section.name} position="top">
							<div
								className={`relative h-full ${isLast ? 'rounded-r' : ''}`}
								style={{
									width: getPercentage(section.value),
									minWidth: '2px', // Ensure very small sections are visible
									backgroundColor: isHovered
										? lightenColor(section.color, 30)
										: section.color,
									transition: 'background-color 0.3s ease-in-out'
								}}
								onMouseEnter={() => setHoveredSectionIndex(index)}
								onMouseLeave={() => setHoveredSectionIndex(null)}
							/>
						</Tooltip>
					);
				})}
			</div>
			<div className={`mt-6 flex flex-wrap ${isDark ? 'text-ink-dull' : 'text-gray-800'}`}>
				{sections.map((section, index) => (
					<Tooltip key={index} label={section.tooltip} position="top">
						<div
							className="mb-2 mr-6 flex items-center"
							onMouseEnter={() => setHoveredSectionIndex(index)}
							onMouseLeave={() => setHoveredSectionIndex(null)}
						>
							<span
								className="mr-2 inline-block size-2 rounded-full"
								style={{ backgroundColor: section.color }}
							/>
							<span className="text-sm">{section.name}</span>
						</div>
					</Tooltip>
				))}
			</div>
		</div>
	);
};

export default StorageBar;
