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

  return (
    <>
      {props.file.is_dir ? (
        <img
          className={clsx('mt-0.5 pointer-events-none z-90', props.className)}
          src="/svg/folder.svg"
        />
      ) : (
        props.file.has_local_thumbnail &&
        client?.data_path && (
          <img
            className="mt-0.5 pointer-events-none z-90"
            src={convertFileSrc(
              `${client.data_path}/thumbnails/${props.locationId}/${props.file.temp_checksum}.webp`
            )}
          />
        )
      )}
    </>
  );
}
