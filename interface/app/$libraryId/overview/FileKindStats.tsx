import React, { useState, useEffect, useRef, useCallback, MouseEventHandler } from 'react';
import { Card, Tooltip } from '@sd/ui';
import { useIsDark } from '~/hooks';
import { getIcon } from '@sd/assets/util';
import { useLibraryQuery } from '@sd/client';
import { useNavigate } from 'react-router';
import { useLocale } from '~/hooks';
import { Info } from '@phosphor-icons/react';
import { motion } from 'framer-motion';

const INFO_ICON_CLASSLIST = "inline size-3 text-ink-faint opacity-0";
const TOTAL_FILES_CLASSLIST = "flex items-center justify-between whitespace-nowrap text-sm font-medium text-ink-dull mt-2 px-1";
const UNIDENTIFIED_FILES_CLASSLIST = "relative flex items-center text-xs text-ink-faint";

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

interface FileKindStatsProps {}

const FileKindStats: React.FC<FileKindStatsProps> = () => {
  const isDark = useIsDark();
  const navigate = useNavigate();
  const { t } = useLocale();
  const { data } = useLibraryQuery(['library.kindStatistics']);
  const [fileKinds, setFileKinds] = useState<FileKind[]>([]);
  const [cardWidth, setCardWidth] = useState<number>(0);
  const containerRef = useRef<HTMLDivElement>(null);
  const iconsRef = useRef<{ [key: string]: HTMLImageElement }>({});

  const BARHEIGHT = 115;
  const BARCOLOR_START = '#3A7ECC';
  const BARCOLOR_END = '#004C99';

  const formatCount = (count: number) => (count >= 1000 ? (count / 1000).toFixed(0) + 'k' : count.toString());

  const handleResize = useCallback(() => {
    if (containerRef.current) {
      const factor = window.innerWidth > 1500 ? 0.35 : 0.4;
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
        .filter((item) => item.kind !== 0 && item.count !== 0)
        .sort((a, b) => b.count - a.count);

      setFileKinds(statistics.map((item) => ({ kind: item.name, count: item.count, id: item.kind })));

      statistics.forEach((item) => {
        const iconName = item.name;
        if (!iconsRef.current[iconName]) {
          const img = new Image();
          img.src = getIcon(iconName + "20", isDark);
          iconsRef.current[iconName] = img;
        }
      });
    }
  }, [data, isDark]);

  const sortedFileKinds = [...fileKinds].sort((a, b) => b.count - a.count);
  let maxFileCount: number;
  if (sortedFileKinds && sortedFileKinds[0]) {
  maxFileCount = sortedFileKinds.length > 0 ? sortedFileKinds[0].count : 0;
  }

  const getPercentage = (value: number) => `${((value / maxFileCount) * BARHEIGHT).toFixed(2)}px`;

  const barGap = 12;
  const barCount = sortedFileKinds.length;
  const totalGapWidth = barGap * (barCount - 5);
  const barWidth = barCount > 0 ? (cardWidth - totalGapWidth) / barCount : 0;

  const formatNumberWithCommas = (number: number) => number.toLocaleString();

  const handleBarClick = (fileKind: FileKind): MouseEventHandler<HTMLDivElement> | undefined => () => {
    const path = {
      pathname: '../search',
      search: new URLSearchParams({
        filters: JSON.stringify([{ object: { kind: { in: [fileKind.id] } } }])
      }).toString()
    };
    navigate(path);
  };

  return (
    <div className="flex justify-center">
      <Card ref={containerRef} className="max-w-1/2 group mx-1 flex h-[220px] w-full min-w-[400px] shrink-0 flex-col bg-app-box/50">
        <div className={TOTAL_FILES_CLASSLIST}>
          <Tooltip className="flex items-center" label={t("bar_graph_info")}>
            <div className="flex items-center gap-2">
              <span className={`${isDark ? "text-white" : "text-black"} text-xl font-black`}>
                {data?.total_identified_files ? formatNumberWithCommas(data.total_identified_files) : "0"}{" "}
              </span>
			  <div className="flex items-center">
			  {t("total_files")}
			  <Info weight="fill" className={`ml-1 ${INFO_ICON_CLASSLIST} opacity-0 transition-opacity duration-300 group-hover:opacity-70`} />
			  </div>
            </div>
          </Tooltip>
          <div className={UNIDENTIFIED_FILES_CLASSLIST}>
            <Tooltip label={t("unidentified_files_info")}>
              <span>{data?.total_unidentified_files ? formatNumberWithCommas(data.total_unidentified_files - data.total_identified_files) : "0"} unidentified files</span>
            </Tooltip>
          </div>
        </div>
        <div className="relative flex grow items-end justify-center">
          {sortedFileKinds.map((fileKind, index) => {
            const icon = iconsRef.current[fileKind.kind];
            const colorFactor = index / (barCount - 1);
            const barColor = interpolateColor(BARCOLOR_START, BARCOLOR_END, colorFactor);

            return (
              <Tooltip key={fileKind.kind} label={formatNumberWithCommas(fileKind.count) + " " + fileKind.kind + "s"} position="left">
                <div
                  className="relative flex min-w-6 max-w-10 grow cursor-pointer flex-col items-center"
                  style={{
                    width: `${barWidth}px`,
                    marginLeft: index === 0 ? 0 : `${barGap}px`,
                  }}
                  onDoubleClick={handleBarClick(fileKind)}
                >
                  {icon && (
                    <img
                      src={icon.src}
                      alt={fileKind.kind}
                      className="relative mb-1 size-4 duration-500"
                    />
                  )}
                  <motion.div
                    className="flex w-full flex-col items-center rounded transition-all duration-500"
                    initial={{ height: 0 }}
                    animate={{ height: getPercentage(fileKind.count) }}
                    transition={{ duration: 0.4, ease: [0.42, 0, 0.58, 1] }}
                    style={{
                      height: getPercentage(fileKind.count),
                      minHeight: '2px',
                      backgroundColor: barColor,
                    }}
                  ></motion.div>
                  <div
                    className="sm mt-1 text-[10px] font-medium text-ink-faint"
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
