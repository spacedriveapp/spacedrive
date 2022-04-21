import { useBridgeQuery } from '@sd/client';
import { FilePath } from '@sd/core';
import clsx from 'clsx';
import React, { useContext } from 'react';
import { AppPropsContext } from '../../App';

import { ReactComponent as Folder } from '../../assets/svg/folder.svg';

export default function FileThumb(props: {
  file: FilePath;
  locationId: number;
  className?: string;
}) {
  const appPropsContext = useContext(AppPropsContext);
  const { data: client } = useBridgeQuery('ClientGetState');

  if (props.file.is_dir) {
    return <Folder className="" />;
  }

  if (props.file.has_local_thumbnail && client?.data_path) {
    return (
      <img
        className="mt-0.5 pointer-events-none z-90"
        src={appPropsContext?.convertFileSrc(
          `${client.data_path}/thumbnails/${props.locationId}/${props.file.temp_cas_id}.webp`
        )}
      />
    );
  }

  return <div></div>;
}
