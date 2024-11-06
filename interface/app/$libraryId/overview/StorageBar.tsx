import clsx from 'clsx';
import React, { useEffect, useRef, useState } from 'react';
import { Tooltip } from '@sd/ui';
import { useIsDark } from '~/hooks';

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
	const [containerWidth, setContainerWidth] = useState(0);
	const containerRef = useRef<HTMLDivElement>(null);

	useEffect(() => {
		if (!containerRef.current) return;

		const resizeObserver = new ResizeObserver((entries) => {
			for (const entry of entries) {
				setContainerWidth(entry.contentRect.width);
			}
		});

		resizeObserver.observe(containerRef.current);
		return () => resizeObserver.disconnect();
	}, []);

	const totalSpace = sections.reduce((acc, section) => acc + section.value, 0n);

	const getWidth = (value: bigint) => {
		if (value === 0n) return '2px';
		const percentage = Number((value * 100n) / totalSpace) / 100;
		return `${Math.max(2, containerWidth * percentage)}px`;
	};

	return (
		<div ref={containerRef} className="w-full p-3 font-plex">
			<div className="relative mt-1 flex h-3 w-full rounded bg-app-box dark:bg-gray-800">
				{sections.map((section, index) => {
					const isHovered = hoveredSectionIndex === index;

					return (
						<Tooltip key={index} label={section.name} position="top">
							<div
								className={clsx('relative h-full', {
									'rounded-l': index === 0,
									'rounded-r': index === sections.length - 1
								})}
								style={{
									width: getWidth(section.value),
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
				className={clsx('mt-3 flex gap-6 px-1', isDark ? 'text-ink-dull' : 'text-gray-800')}
			>
				{sections.map((section, index) => (
					<Tooltip key={index} label={section.tooltip} position="top">
						<div
							className="flex items-center"
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
