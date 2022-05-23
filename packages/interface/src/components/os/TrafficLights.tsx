import clsx from 'clsx';
import React from 'react';

import { ReactComponent as Close } from '../../assets/svg/macos_close.svg';
import { ReactComponent as Fullscreen } from '../../assets/svg/macos_fullscreen.svg';
import { ReactComponent as Minimize } from '../../assets/svg/macos_minimize.svg';
import { useFocusState } from '../../hooks/useFocusState';
import { DefaultProps } from '../primitive/types';

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
			<Light mode="close" action={props.onClose} focused={focused} />
			<Light mode="minimize" action={props.onMinimize} focused={focused} />
			<Light mode="fullscreen" action={props.onFullscreen} focused={focused} />
		</div>
	);
};

interface LightProps {
	mode: 'close' | 'minimize' | 'fullscreen';
	focused: boolean;
	action?: () => void;
}

const Light: React.FC<LightProps> = (props) => {
	return (
		<div
			onClick={props.action}
			className={clsx('w-[13px] h-[13px] rounded-full bg-gray-500', {
				'!bg-red-400 active:!bg-red-300': props.mode == 'close' && props.focused,
				'!bg-green-400 active:!bg-green-300': props.mode == 'fullscreen' && props.focused,
				'!bg-yellow-400 active:!bg-yellow-300': props.mode == 'minimize' && props.focused
			})}
		>
			{(() => {
				switch (props.mode) {
					case 'close':
						return (
							<Close className=" w-[13px] -mt-[1px] h-[15px] opacity-0 group-hover:opacity-100" />
						);
					case 'minimize':
						return (
							<Minimize className="ml-[2px] w-[9px] -mt-[1px] h-[15px] opacity-0 group-hover:opacity-100" />
						);
					case 'fullscreen':
						return (
							<Fullscreen className="ml-[1px] w-[11px] -mt-[1px] h-[15px]  opacity-0 group-hover:opacity-100" />
						);
				}
			})()}
		</div>
	);
};
