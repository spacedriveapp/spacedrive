import { useBridgeQuery } from '@sd/client';
import { FilePath } from '@sd/core';
import clsx from 'clsx';
import React, { useContext } from 'react';
import { AppPropsContext } from '../../App';
import icons from '../../assets/icons';
import { ReactComponent as Folder } from '../../assets/svg/folder.svg';

export default function FileThumb(props: {
  file: FilePath;
  locationId: number;
  hasThumbnailOverride: boolean;
  className?: string;
}) {
  const appPropsContext = useContext(AppPropsContext);
  const { data: client } = useBridgeQuery('ClientGetState');

  if (props.file.is_dir) {
    return <Folder className="max-w-[170px]" />;
  }

  if (client?.data_path && (props.file.has_local_thumbnail || props.hasThumbnailOverride)) {
    return (
      <img
        className="pointer-events-none z-90"
        src={appPropsContext?.convertFileSrc(
          `${client.data_path}/thumbnails/${props.locationId}/${props.file.temp_cas_id}.webp`
        )}
      />
    );
  }

  if (icons[props.file.extension as keyof typeof icons]) {
    const Icon = icons[props.file.extension as keyof typeof icons];
    return <Icon className={clsx('max-w-[170px] w-full h-full', props.className)} />;
  }
  return <div></div>;
}
