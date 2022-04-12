import { useBridgeQuery } from '@sd/client';
import { FilePath } from '@sd/core';
import { convertFileSrc } from '@tauri-apps/api/tauri';
import clsx from 'clsx';
import React from 'react';

export default function FileThumb(props: {
  file: FilePath;
  locationId: number;
  className?: string;
}) {
  const { data: client } = useBridgeQuery('ClientGetState');

  if (props.file.is_dir) {
    return (
      <img
        className={clsx('mt-0.5 pointer-events-none z-90', props.className)}
        src="/svg/folder.svg"
      />
    );
  }

  if (props.file.has_local_thumbnail && client?.data_path) {
    return (
      <img
        className="mt-0.5 pointer-events-none z-90"
        src={convertFileSrc(
          `${client.data_path}/thumbnails/${props.locationId}/${props.file.temp_cas_id}.webp`
        )}
      />
    );
  }

  return <div></div>;
}
