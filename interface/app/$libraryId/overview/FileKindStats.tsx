import React, { useState, useEffect, useRef, useCallback, MouseEventHandler } from 'react';
import { Card, Tooltip } from '@sd/ui';
import { useIsDark } from '~/hooks';
import { getIcon } from '@sd/assets/util';
import { useLibraryQuery } from '@sd/client';
import { useNavigate } from 'react-router';
import { useLocale } from '~/hooks';
import { Info } from '@phosphor-icons/react';

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
  const [fileKinds, setFileKinds] = useState<FileKind[]>([]);
  const [cardWidth, setCardWidth] = useState<number>(0);
  const containerRef = useRef<HTMLDivElement>(null);
  const iconsRef = useRef<{ [key: string]: HTMLImageElement }>({});

  const BARHEIGHT = 140;
  const BARCOLOR_START = isDark ? '#742AEA' : '#75B1F9';
  const BARCOLOR_END = isDark ? '#FE9BFE' : '#3A7ECC';

  console.log(data);
  const handleResize = useCallback(() => {
    if (containerRef.current) {
      let factor;
      if (window.innerWidth > 2000) {
        factor = 0.60;
      } else if (window.innerWidth > 1600) {
        factor = 0.50;
      } else {
        factor = 0.45;
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
          img.src = getIcon(iconName, isDark);
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
      <Card ref={containerRef} className="group flex h-[220px] w-full min-w-[400px] max-w-full shrink-0 flex-col bg-app-box/50">
        <div className="flex items-center justify-end whitespace-nowrap text-sm font-medium text-ink-dull">
          <div><span className={isDark ? "text-white" : "text-black"}>{totalFiles + " "}</span>{t("total_files")}</div>
          <Tooltip label={t("bar_graph_info")}>
            <Info
              weight="fill"
              className="mb-1 ml-1 inline size-3 text-ink-faint opacity-0 transition-opacity duration-300 group-hover:opacity-70"
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
                      className="relative bottom-1 size-6 transition-all duration-500"
                    />
                  )}
                  <div
                    className="flex w-full flex-col items-center rounded transition-all duration-500"
                    style={{
                      height: getPercentage(fileKind.count),
                      minHeight: '2px',
                      backgroundColor: barColor,
                    }}
                  >
                  </div>
                  <div
                    className="sm my-1 text-[10px] font-medium text-ink-faint"
                    style={{
                      borderRadius: '3px',
                    }}
                  >
                    {fileKind.count}
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
