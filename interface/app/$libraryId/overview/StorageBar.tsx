import React, { useState } from 'react';
import { Tooltip } from '@sd/ui'; // Ensure you import your Tooltip component correctly
import { humanizeSize } from '@sd/client';
import { useIsDark} from '~/hooks';

//change this to dynamically change the width of the bar (needed since each section's width is calculated independently)
const BARWIDTH = 520

// Function to lighten color
const lightenColor = (color: string, percent: number) => {
  const num = parseInt(color.replace("#", ""), 16);
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
  value: number;
  color: string;
  tooltip: string;
}

interface StorageBarProps {
  sections: Section[];
  totalSpace: number;
}

const StorageBar: React.FC<StorageBarProps> = ({ sections, totalSpace }) => {
  const isDark = useIsDark();
  const [hoveredSectionIndex, setHoveredSectionIndex] = useState<number | null>(null);

  const getPercentage = (value: number) => {
    const percentage = (value / totalSpace);
    const pixvalue = BARWIDTH * percentage;
    return `${pixvalue.toFixed(2)}px`;
  };

  const usedSpace = sections.reduce((acc, section) => acc + section.value, 0);
  const unusedSpace = totalSpace - usedSpace;

  return (
    <div className="w-auto p-3">
      <div className="relative mt-1 flex h-6 overflow-hidden rounded">
        {sections.map((section, index) => {
          const humanizedValue = humanizeSize(section.value);
          const isHovered = hoveredSectionIndex === index;

          return (
            <Tooltip key={index} label={`${humanizedValue.value} ${humanizedValue.unit}`} position="top">
              <div
                className={`relative h-full`}
                style={{
                  width: getPercentage(section.value),
                  minWidth: '2px', // Ensure very small sections are visible
                  backgroundColor: isHovered ? lightenColor(section.color, 30) : section.color,
                  transition: 'background-color 0.3s ease-in-out',
                }}
                onMouseEnter={() => setHoveredSectionIndex(index)}
                onMouseLeave={() => setHoveredSectionIndex(null)}
              />
            </Tooltip>
          );
        })}
        {unusedSpace > 0 && (
          <div
            className="relative h-full rounded-r"
            style={{
              width: getPercentage(unusedSpace),
              backgroundColor: isDark ? '#1C1D25' : '#D3D3D3',
            }}
          />
        )}
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
