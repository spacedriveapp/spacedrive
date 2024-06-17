import React, { useState, useEffect, useRef, useCallback, MouseEventHandler } from 'react';
import { Card, Tooltip } from '@sd/ui';
import { useIsDark } from '~/hooks';
import { getIcon } from '@sd/assets/util';
import { useLibraryQuery } from '@sd/client';
import { useNavigate } from 'react-router';
import { useLocale } from '~/hooks';
import { Info } from '@phosphor-icons/react';
import { motion } from 'framer-motion';

const interpolateColor = (color1: string, color2: string, factor: number) => {
  const hex = (color: string) => parseInt(color.slice(1), 16);
  const r = Math.round((1 - factor) * (hex(color1) >> 16) + factor * (hex(color2) >> 16));
  const g = Math.round((1 - factor) * ((hex(color1) >> 8) & 0x00ff) + factor * ((hex(color2) >> 8) & 0x00ff));
  const b = Math.round((1 - factor) * (hex(color1) & 0x0000ff) + factor * (hex(color2) & 0x0000ff));
  return `#${((1 << 24) + (r << 16) + (g << 8) + b).toString(16).slice(1).toUpperCase()}`;
};

interface KindStatistic {
  kind: number;
  name: string;
  count: number;
}

interface FileKind {
  kind: string;
  count: number;
  id: number;
}

interface FileKindStatsProps {
  // Define the props for your component here
}

const FileKindStats: React.FC<FileKindStatsProps> = () => {
  const isDark = useIsDark();
  const navigate = useNavigate();
  const { t } = useLocale();
  const { data } = useLibraryQuery(['library.kindStatistics']);
  const [fileKinds, setFileKinds] = useState<FileKind[]>([
    { kind: 'Documents', count: 500, id: 1 },
    { kind: 'Images', count: 300, id: 2 },
    { kind: 'Videos', count: 100, id: 3 },
  ]);
  const [cardWidth, setCardWidth] = useState<number>(0);
  const containerRef = useRef<HTMLDivElement>(null);
  const iconsRef = useRef<{ [key: string]: HTMLImageElement }>({});

  const BARHEIGHT = 100;
  const BARCOLOR_START = isDark ? '#3A7ECC' : '#004C99';
  const BARCOLOR_END = isDark ? '#004C99' : '#3A7ECC';

  const formatCount = (count: number) => {
    if (count >= 1000) {
      return (count / 1000).toFixed(1) + 'k';
    }
    return count.toString();
  };

  const handleResize = useCallback(() => {
    if (containerRef.current) {
      let factor;
      if (window.innerWidth > 1500) {
        factor = 0.35;
      } else {
        factor = 0.4;
      }
      setCardWidth(window.innerWidth * factor);
    }
  }, []);

  useEffect(() => {
    window.addEventListener('resize', handleResize);
    handleResize();

    const containerElement = containerRef.current;
    if (containerElement) {
      const observer = new MutationObserver(handleResize);
      observer.observe(containerElement, {
        attributes: true,
        childList: true,
        subtree: true,
        attributeFilter: ['style'],
      });

      return () => {
        observer.disconnect();
      };
    }

    return () => {
      window.removeEventListener('resize', handleResize);
    };
  }, [handleResize]);

  useEffect(() => {
    if (data) {
      const statistics: KindStatistic[] = data.statistics
        .filter((item: KindStatistic) => item.count !== 0)
        .sort((a: KindStatistic, b: KindStatistic) => b.count - a.count);

      setFileKinds(statistics.map(item => ({ kind: item.name, count: item.count, id: item.kind })));

      statistics.forEach(item => {
        const iconName = item.name;
        if (!iconsRef.current[iconName]) {
          const img = new Image();
          img.src = getIcon(iconName + "20", isDark);
          iconsRef.current[iconName] = img;
        }
      });
    }
  }, [data, isDark]);

  const totalFiles = fileKinds.reduce((acc, fileKind) => acc + fileKind.count, 0);
  const sortedFileKinds = [...fileKinds].sort((a, b) => b.count - a.count);
  let maxFileCount: number;
  if (sortedFileKinds && sortedFileKinds[0]) {
    maxFileCount = sortedFileKinds.length > 0 ? sortedFileKinds[0].count : 0;
  }

  const getPercentage = (value: number) => {
    const percentage = (value / maxFileCount);
    const pixvalue = BARHEIGHT * percentage;
    return `${pixvalue.toFixed(2)}px`;
  };

  const barGap = 12;
  const barCount = sortedFileKinds.length;
  const totalGapWidth = barGap * (barCount - 5);
  const barWidth = barCount > 0 ? (cardWidth - totalGapWidth) / barCount : 0;

  return (
    <div className="flex justify-center">
      <Card ref={containerRef} className="max-w-1/2 group flex h-[220px] w-full min-w-[400px] shrink-0 flex-col bg-app-box/50">
        <div className="mb-4 mt-2 flex items-center whitespace-nowrap text-sm font-medium text-ink-dull">
          <div><span className={isDark ? "mr-1 text-xl text-white" : "text-black"}>{totalFiles + " "}</span>{t("total_files")}</div>
          <Tooltip label={t("bar_graph_info")}>
            <Info
              weight="fill"
              className="ml-1 inline size-3 text-ink-faint opacity-0 transition-opacity duration-300 group-hover:opacity-70"
            />
          </Tooltip>
        </div>
        <div className="relative flex grow items-end justify-center">
          {sortedFileKinds.map((fileKind, index) => {
            const icon = iconsRef.current[fileKind.kind];

            const colorFactor = index / (barCount - 1);
            const barColor = interpolateColor(BARCOLOR_START, BARCOLOR_END, colorFactor);

            const handleBarClick = (kind: any): MouseEventHandler<HTMLDivElement> | undefined => {
              console.log(kind);
              console.log(fileKind.id);
              const path = {
                pathname: '../search',
                search: new URLSearchParams({
                  filters: JSON.stringify([{ object: { kind: { in: [fileKind.id] } } }])
                }).toString()
              };
              navigate(path);
              return;
            }

            return (
              <Tooltip key={fileKind.kind} label={fileKind.kind} position="left">
                <div
                  className="relative flex grow cursor-pointer flex-col items-center transition-all duration-500"
                  style={{
                    width: `${barWidth}px`,
                    marginLeft: index === 0 ? 0 : `${barGap}px`,
                  }}
                  onDoubleClick={handleBarClick}
                >
                  {icon && (
                    <img
                      src={icon.src}
                      alt={fileKind.kind}
                      className="relative bottom-1 mb-1 size-4 transition-all duration-500"
                    />
                  )}
                  <motion.div
                    className="flex w-full flex-col items-center rounded transition-all duration-500"
					initial={{ height: 0 }}
                    animate={{ height: getPercentage(fileKind.count)}}
                    transition={{ duration: 0.4 }}
                    style={{
                      height: getPercentage(fileKind.count),
                      minHeight: '2px',
                      backgroundColor: barColor,
                    }}
                  >
                  </motion.div>
                  <div
                    className="sm my-1 text-[10px] font-medium text-ink-faint"
                    style={{
                      borderRadius: '3px',
                    }}
                  >
                    {formatCount(fileKind.count)}
                  </div>
                </div>
              </Tooltip>
            );
          })}
        </div>
      </Card>
    </div>
  );
};

export default FileKindStats;
