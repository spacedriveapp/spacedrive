import { Info } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useState } from 'react';
import { humanizeSize, Statistics, useLibraryContext, useLibraryQuery } from '@sd/client';
import { Tooltip, Card } from '@sd/ui';
import { useCounter, useLocale, useIsDark } from '~/hooks';
import StorageBar from './StorageBar';

interface StatItemProps {
  title: string;
  bytes: bigint;
  isLoading: boolean;
  info?: string;
}

interface Section {
	name: string;
	value: number;
	color: string;
	tooltip: string;
  }


let mounted = false;

const StatItem = (props: StatItemProps) => {
  const { title, bytes, isLoading } = props;

  const [isMounted] = useState(mounted);

  const size = humanizeSize(bytes);
  const count = useCounter({
    name: title,
    end: size.value,
    duration: isMounted ? 0 : 1,
    saveState: false
  });

  const { t } = useLocale();

  return (
    <div
      className={clsx(
        'group/stat flex w-36 shrink-0 flex-col duration-75',
        !bytes && 'hidden'
      )}
    >
      <span className="whitespace-nowrap text-sm font-medium text-ink-faint">
        {title}
        {props.info && (
          <Tooltip label={props.info}>
            <Info
              weight="fill"
              className="-mt-0.5 ml-1 inline size-3 text-ink-faint opacity-0 transition-opacity duration-300 group-hover/stat:opacity-70"
            />
          </Tooltip>
        )}
      </span>

      <span className="text-2xl">
        <div
          className={clsx({
            hidden: isLoading
          })}
        >
          <span className="font-black tabular-nums">{count}</span>
          <span className="ml-1 text-[16px] font-medium text-ink-faint">
            {t(`size_${size.unit.toLowerCase()}`)}
          </span>
        </div>
      </span>
    </div>
  );
};

const LibraryStats = () => {
const isDark = useIsDark();
  const { library } = useLibraryContext();
  const stats = useLibraryQuery(['library.statistics']);
  const { t } = useLocale();

  useEffect(() => {
    if (!stats.isLoading) mounted = true;
  }, [stats.isLoading]);

  const StatItemNames: Partial<Record<keyof Statistics, string>> = {
    total_library_bytes: t('library_bytes'),
    library_db_size: t('library_db_size'),
    total_local_bytes_capacity: t('total_bytes_capacity'),
    total_library_preview_media_bytes: t('preview_media_bytes'),
    total_local_bytes_free: t('total_bytes_free'),
    total_local_bytes_used: t('total_bytes_used')
  };

  const StatDescriptions: Partial<Record<keyof Statistics, string>> = {
	total_library_bytes: t('library_bytes_description'),
	library_db_size: t('library_db_size_description'),
    total_local_bytes_capacity: t('total_bytes_capacity_description'),
    total_library_preview_media_bytes: t('preview_media_bytes_description'),
    total_local_bytes_free: t('total_bytes_free_description'),
    total_local_bytes_used: t('total_bytes_used_description')
  };

  const displayableStatItems = Object.keys(
    StatItemNames
  ) as unknown as keyof typeof StatItemNames;

  if (!stats.data || !stats.data.statistics) {
    return <div>Loading...</div>;
  }

  const { statistics } = stats.data;
  const totalSpace = Number(statistics.total_local_bytes_capacity);

  const sections = [
	{
	  name: StatItemNames.library_db_size,
	  value: Number(statistics.library_db_size),
	  color: '#75B1F9', // Light mode gradient start
	  tooltip: StatDescriptions.library_db_size,
	},
	{
	  name: StatItemNames.total_library_preview_media_bytes,
	  value: Number(statistics.total_library_preview_media_bytes),
	  color: '#3A7ECC',
	  tooltip: StatDescriptions.total_library_preview_media_bytes,
	},
	{
	  name: StatItemNames.total_local_bytes_used,
	  value: Number(statistics.total_local_bytes_used),
	  color: '#004C99',
	  tooltip: StatDescriptions.total_local_bytes_used,
	},
  ];

  const excludedKeys = ['library_db_size', 'total_library_preview_media_bytes', 'total_local_bytes_used'];

  return (
    <Card className="flex h-[220px] w-[572px] shrink-0 flex-col bg-app-box/50">
      <div className="mb-1 flex gap-4 overflow-hidden p-4">
        {Object.entries(statistics)
          .filter(([key]) => !excludedKeys.includes(key))
          // sort the stats by the order of the displayableStatItems
          .sort(
            ([a], [b]) =>
              displayableStatItems.indexOf(a) - displayableStatItems.indexOf(b)
          )
          .map(([key, value]) => {
            if (!displayableStatItems.includes(key)) return null;
            return (
              <StatItem
                key={`${library.uuid} ${key}`}
                title={StatItemNames[key as keyof Statistics]!}
                bytes={BigInt(value as number)}
                isLoading={stats.isLoading}
                info={StatDescriptions[key as keyof Statistics]}
              />
            );
          })}
      </div>
      <div>
        <StorageBar sections={sections  as Section[]} totalSpace={totalSpace} />
      </div>
    </Card>
  );
};

export default LibraryStats;
