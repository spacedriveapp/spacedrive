import clsx from 'clsx';
import React, { useEffect } from 'react';
import { useFocusState } from '../../hooks/useFocusState';
import { DefaultProps } from '../primitive/types';

import closeIconPath from '../../assets/svg/macos_close.svg';
import minimizeIconPath from '../../assets/svg/macos_minimize.svg';
import fullscreenIconPath from '../../assets/svg/macos_fullscreen.svg';

export interface TrafficLightsProps extends DefaultProps {
  onClose?: () => void;
  onMinimize?: () => void;
  onFullscreen?: () => void;
}

export const TrafficLights: React.FC<TrafficLightsProps> = (props) => {
  const [focused] = useFocusState();
  return (
    <div
      data-tauri-drag-region
      className={clsx('flex flex-row space-x-2 px-2 group', props.className)}
    >
      <TrafficLight type="close" onClick={props.onClose} colorful={focused} />
      <TrafficLight type="minimize" onClick={props.onMinimize} colorful={focused} />
      <TrafficLight type="fullscreen" onClick={props.onFullscreen} colorful={focused} />
    </div>
  );
};

interface TrafficLightProps {
  type: 'close' | 'minimize' | 'fullscreen';
  colorful: boolean;
  onClick?: () => void;
}

const TrafficLight: React.FC<TrafficLightProps> = (props) => {
  const { onClick = () => undefined, colorful = false, type } = props;
  const iconPath = React.useRef<string>(closeIconPath);

  useEffect(() => {
    switch (type) {
      case 'close':
        iconPath.current = closeIconPath;
        break;
      case 'minimize':
        iconPath.current = minimizeIconPath;
        break;
      case 'fullscreen':
        iconPath.current = fullscreenIconPath;
        break;
    }
  }, [type]);

  return (
    <div
      onClick={onClick}
      className={clsx(
        'rounded-full box-content w-[12px] h-[12px] border-[0.5px] border-transparent bg-[#CDCED0] dark:bg-[#2B2C2F] flex justify-center items-center',
        {
          'border-red-900 !bg-[#EC6A5E] active:hover:!bg-red-700 dark:active:hover:!bg-red-400':
            type === 'close' && colorful,
          'border-yellow-900 !bg-[#F4BE4F] active:hover:!bg-yellow-600 dark:active:hover:!bg-yellow-200':
            type === 'minimize' && colorful,
          'border-green-900 !bg-[#61C253] active:hover:!bg-green-700 dark:active:hover:!bg-green-300':
            type === 'fullscreen' && colorful
        }
      )}
    >
      {colorful && (
        <img
          src={iconPath.current}
          className="opacity-0 group-hover:opacity-100 group-active:opacity-100 pointer-events-none"
        />
      )}
    </div>
  );
};
