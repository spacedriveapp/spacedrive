import { ChevronLeftIcon, ChevronRightIcon } from '@heroicons/react/outline';
import { useBridgeCommand } from '@sd/client';
import { Dropdown } from '@sd/ui';
import clsx from 'clsx';
import {
	ArrowsClockwise,
	Cloud,
	FolderPlus,
	IconProps,
	Key,
	List,
	Tag,
	TerminalWindow
} from 'phosphor-react';
import React, { DetailedHTMLProps, HTMLAttributes } from 'react';
import { useNavigate } from 'react-router-dom';

import { useExplorerState } from '../../hooks/useExplorerState';
import { Shortcut } from '../primitive/Shortcut';
import { DefaultProps } from '../primitive/types';

export interface TopBarProps extends DefaultProps {}
export interface TopBarButtonProps
	extends DetailedHTMLProps<HTMLAttributes<HTMLButtonElement>, HTMLButtonElement> {
	icon: React.ComponentType<IconProps>;
	group?: boolean;
	active?: boolean;
	left?: boolean;
	right?: boolean;
}

const TopBarButton: React.FC<TopBarButtonProps> = ({ icon: Icon, ...props }) => {
	return (
		<button
			{...props}
			className={clsx(
				'mr-[1px] py-0.5 px-0.5 text-md font-medium hover:bg-gray-150 dark:transparent dark:hover:bg-gray-550 dark:active:bg-gray-500 rounded-md transition-colors duration-100',
				{
					'rounded-r-none rounded-l-none': props.group && !props.left && !props.right,
					'rounded-r-none': props.group && props.left,
					'rounded-l-none': props.group && props.right,
					'dark:bg-gray-450 dark:hover:bg-gray-450 dark:active:bg-gray-450': props.active
				},
				props.className
			)}
		>
			<Icon weight={'regular'} className="m-0.5 w-5 h-5 text-gray-450 dark:text-gray-150" />
		</button>
	);
};

export const TopBar: React.FC<TopBarProps> = (props) => {
	const { locationId } = useExplorerState();
	const { mutate: generateThumbsForLocation } = useBridgeCommand('GenerateThumbsForLocation', {
		onMutate: (data) => {
			console.log('GenerateThumbsForLocation', data);
		}
	});

	const { mutate: identifyUniqueFiles } = useBridgeCommand('IdentifyUniqueFiles', {
		onMutate: (data) => {
			console.log('IdentifyUniqueFiles', data);
		},
		onError: (error) => {
			console.error('IdentifyUniqueFiles', error);
		}
	});

	let navigate = useNavigate();
	return (
		<>
			<div
				data-tauri-drag-region
				className="flex h-[2.95rem] -mt-0.5 max-w z-10 pl-3 flex-shrink-0 items-center border-b  dark:bg-gray-600 border-gray-100 dark:border-gray-800 !bg-opacity-60 backdrop-blur"
			>
				<div className="flex">
					<TopBarButton icon={ChevronLeftIcon} onClick={() => navigate(-1)} />
					<TopBarButton icon={ChevronRightIcon} onClick={() => navigate(1)} />
				</div>
				{/* <div className="flex mx-8 space-x-[1px]">
          <TopBarButton active group left icon={List} />
          <TopBarButton group icon={Columns} />
          <TopBarButton group right icon={SquaresFour} />
        </div> */}
				<div data-tauri-drag-region className="flex flex-row justify-center flex-grow ">
					<div className="flex mx-8 space-x-2 pointer-events-auto">
						<TopBarButton icon={Tag} />
						<TopBarButton icon={FolderPlus} />
						<TopBarButton icon={TerminalWindow} />
					</div>
					<div className="relative flex h-7">
						<input
							placeholder="Search"
							className="w-32 h-[30px] focus:w-52 text-sm p-3 rounded-lg outline-none focus:ring-2  placeholder-gray-400 dark:placeholder-gray-500 bg-[#F6F2F6] border border-gray-50 dark:bg-gray-650 dark:border-gray-550 focus:ring-gray-100 dark:focus:ring-gray-600 transition-all"
						/>
						<div className="space-x-1 absolute top-[2px] right-1">
							<Shortcut chars="⌘K" />
							{/* <Shortcut chars="S" /> */}
						</div>
					</div>
					<div className="flex mx-8 space-x-2">
						<TopBarButton icon={Key} />
						<TopBarButton icon={Cloud} />
						<TopBarButton
							icon={ArrowsClockwise}
							onClick={() => {
								generateThumbsForLocation({ id: locationId, path: '' });
							}}
						/>
					</div>
				</div>
				{/* <img
          alt="spacedrive-logo"
          src="/images/spacedrive_logo.png"
          className="w-8 h-8 mt-[1px] mr-2 pointer-events-none"
        /> */}
				<div className="flex mr-3 space-x-2">
					<Dropdown
						// className="absolute block h-6 w-44 top-2 right-4"
						items={[
							[
								{
									name: 'Generate Thumbs',
									icon: ArrowsClockwise,
									onPress: () => generateThumbsForLocation({ id: locationId, path: '' })
								},
								{
									name: 'Identify Unique',
									icon: ArrowsClockwise,
									onPress: () => identifyUniqueFiles({ id: locationId, path: '' })
								}
							]
						]}
						buttonComponent={<TopBarButton icon={List} />}
					/>
				</div>
				{/*<TopBarButton onClick={() => {*/}
				{/*  setSettingsOpen(!settingsOpen);*/}
				{/*}} className="mr-[8px]" icon={CogIcon} />*/}
			</div>
			{/* <div className="h-[1px] flex-shrink-0 max-w bg-gray-200 dark:bg-gray-700" /> */}
		</>
	);
};
