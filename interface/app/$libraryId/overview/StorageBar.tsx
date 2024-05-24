import React, { useState } from 'react';
import { Tooltip } from '@sd/ui'; // Ensure you import your Tooltip component correctly

interface Section {
  name: string;
  value: number;
  color: string;
  tooltip: string;
}

interface StorageBarProps {
  sections: Section[];
  totalSpace: number;
}

const StorageBar: React.FC<StorageBarProps> = ({ sections, totalSpace }) => {
  const [hoveredSectionIndex, setHoveredSectionIndex] = useState<number | null>(null);

  const getPercentage = (value: number) => {
    const percentage = (value / totalSpace) * 100;
    return `${percentage.toFixed(2)}%`;
  };

  const usedSpace = sections.reduce((acc, section) => acc + section.value, 0);
  const unusedSpace = totalSpace - usedSpace;

  return (
    <div className="w-full p-4">
      <div className="relative flex h-5 w-full overflow-hidden rounded bg-[#1C1D25]">
        {sections.map((section, index) => (
          <div
            key={index}
            className={`relative h-full ${
              hoveredSectionIndex === index ? 'rounded-sm border border-white' : ''
            }`}
            style={{
              width: getPercentage(section.value),
              minWidth: '3px', // Ensure very small sections are visible
              backgroundColor: section.color,
              marginRight: index < sections.length - 1 ? '1px' : '0', // Add space between sections
            }}
          />
        ))}
        {unusedSpace > 0 && (
          <div
            className="relative h-full grow"
            style={{
              width: getPercentage(unusedSpace),
              backgroundColor: '#1C1D25',
            }}
          />
        )}
      </div>
      <div className="mt-1 flex flex-wrap text-ink-dull">
        {sections.map((section, index) => (
          <Tooltip key={index} label={section.tooltip} position="top">
            <div
              className="mb-2 mr-4 flex items-center"
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
