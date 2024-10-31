import clsx from 'clsx';
import React, { useState } from 'react';
import { Tooltip } from '@sd/ui';
import { useIsDark } from '~/hooks';

const BARWIDTH = 690;

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
		const percentage = Number((value * 100n) / totalSpace) / 100;
		const pixvalue = BARWIDTH * percentage;
		return `${pixvalue.toFixed(2)}px`;
	};

	return (
		<div className="w-auto p-3 font-plex">
			<div className="relative mt-1 flex h-6 rounded">
				{sections.map((section, index) => {
					const isHovered = hoveredSectionIndex === index;

					return (
						<Tooltip key={index} label={section.name} position="top">
							<div
								className={clsx('relative h-full', {
									// Add rounded corners to first and last sections
									'rounded-l': index === 0,
									'rounded-r': index === sections.length - 1
								})}
								style={{
									width: getPercentage(section.value),
									minWidth: '2px', // Ensure very small sections are visible
									backgroundColor: section.color,
									opacity: hoveredSectionIndex === null || isHovered ? 1 : 0.3,
									transition: 'opacity 0.3s ease-in-out'
								}}
								onMouseEnter={() => setHoveredSectionIndex(index)}
								onMouseLeave={() => setHoveredSectionIndex(null)}
							/>
						</Tooltip>
					);
				})}
			</div>
			<div
				className={clsx('mt-6 flex flex-wrap', isDark ? 'text-ink-dull' : 'text-gray-800')}
			>
				{sections.map((section, index) => (
					<Tooltip key={index} label={section.tooltip} position="top">
						<div
							className="mb-2 mr-6 flex items-center"
							onMouseEnter={() => setHoveredSectionIndex(index)}
							onMouseLeave={() => setHoveredSectionIndex(null)}
						>
							<span
								className="mr-2 inline-block size-2 rounded-full"
								style={{
									backgroundColor: section.color
								}}
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
