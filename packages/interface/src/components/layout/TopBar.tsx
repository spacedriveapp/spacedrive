import { ChevronLeftIcon, ChevronRightIcon } from '@heroicons/react/outline';
import { AppPropsContext, useExplorerStore, useLibraryMutation } from '@sd/client';
import { Dropdown } from '@sd/ui';
import clsx from 'clsx';
import { ArrowsClockwise, IconProps, Key, List, Rows, SquaresFour } from 'phosphor-react';
import React, { DetailedHTMLProps, HTMLAttributes, RefAttributes, useContext } from 'react';
import { useNavigate } from 'react-router-dom';

import { Shortcut } from '../primitive/Shortcut';
import { DefaultProps } from '../primitive/types';
import { Tooltip } from '../tooltip/Tooltip';

export type TopBarProps = DefaultProps;
export interface TopBarButtonProps
	extends DetailedHTMLProps<HTMLAttributes<HTMLButtonElement>, HTMLButtonElement> {
	icon: React.ComponentType<IconProps>;
	group?: boolean;
	active?: boolean;
	left?: boolean;
	right?: boolean;
}

const TopBarButton: React.FC<TopBarButtonProps> = ({
	icon: Icon,
	left,
	right,
	group,
	active,
	className,
	...props
}) => {
	return (
		<button
			{...props}
			className={clsx(
				'mr-[1px] py-0.5 px-0.5 text-md font-medium hover:bg-gray-150 dark:transparent dark:hover:bg-gray-550 rounded-md transition-colors duration-100',
				{
					'rounded-r-none rounded-l-none': group && !left && !right,
					'rounded-r-none': group && left,
					'rounded-l-none': group && right,
					'dark:bg-gray-550': active
				},
				className
			)}
		>
			<Icon weight={'regular'} className="m-0.5 w-5 h-5 text-gray-450 dark:text-gray-150" />
		</button>
	);
};

const SearchBar = React.forwardRef<HTMLInputElement, DefaultProps>((props, ref) => {
	//TODO: maybe pass the appProps, so we can have the context in the TopBar if needed again
	const appProps = useContext(AppPropsContext);

	return (
		<div className="relative flex h-7">
			<input
				ref={ref}
				placeholder="Search"
				className="peer w-32 h-[30px] focus:w-52 text-sm p-3 rounded-lg outline-none focus:ring-2  placeholder-gray-400 dark:placeholder-gray-450 bg-[#F6F2F6] border border-gray-50 shadow-md dark:bg-gray-600 dark:border-gray-550 focus:ring-gray-100 dark:focus:ring-gray-550 dark:focus:bg-gray-800 transition-all"
			/>
			<div className="space-x-1 absolute top-[2px] right-1 peer-focus:invisible">
				<Shortcut
					chars={
						appProps?.platform === 'macOS' || appProps?.platform === 'browser' ? '⌘L' : 'CTRL+L'
					}
				/>
				{/* <Shortcut chars="S" /> */}
			</div>
		</div>
	);
});

export const TopBar: React.FC<TopBarProps> = (props) => {
	const { locationId, layoutMode, set } = useExplorerStore();
	const { mutate: generateThumbsForLocation } = useLibraryMutation(
		'jobs.generateThumbsForLocation',
		{
			onMutate: (data) => {
				console.log('GenerateThumbsForLocation', data);
			}
		}
	);

	const { mutate: identifyUniqueFiles } = useLibraryMutation('jobs.identifyUniqueFiles', {
		onMutate: (data) => {
			console.log('IdentifyUniqueFiles', data);
		},
		onError: (error) => {
			console.error('IdentifyUniqueFiles', error);
		}
	});

	const navigate = useNavigate();

	//create function to focus on search box when cmd+k is pressed
	const searchRef = React.useRef<HTMLInputElement>(null);
	React.useEffect(() => {
		const handler = (e: KeyboardEvent) => {
			if (e.metaKey && e.key === 'l') {
				if (searchRef.current) searchRef.current.focus();
				e.preventDefault();
				return;
			}

			if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
				if (e.key === 'Escape') {
					e.target.blur();
					e.preventDefault();
					return;
				}
			} else {
				if (e.key === '/') {
					if (searchRef.current) searchRef.current.focus();
					e.preventDefault();
					return;
				}
			}
		};

		document.addEventListener('keydown', handler);
		return () => document.removeEventListener('keydown', handler);
	}, []);

	return (
		<>
			<div
				data-tauri-drag-region
				className="flex h-[2.95rem] -mt-0.5 max-w z-10 pl-3 flex-shrink-0 items-center  dark:bg-gray-600 border-gray-100 dark:border-gray-800 !bg-opacity-80 backdrop-blur"
			>
				<div className="flex ">
					<Tooltip label="Navigate back">
						<TopBarButton icon={ChevronLeftIcon} onClick={() => navigate(-1)} />
					</Tooltip>
					<Tooltip label="Navigate forward">
						<TopBarButton icon={ChevronRightIcon} onClick={() => navigate(1)} />
					</Tooltip>
				</div>

				{/* <div className="flex mx-8 space-x-[1px]">
          <TopBarButton active group left icon={List} />
          <TopBarButton group icon={Columns} />
          <TopBarButton group right icon={SquaresFour} />
        </div> */}

				<div data-tauri-drag-region className="flex flex-row justify-center flex-grow">
					<div className="flex mx-8">
						<Tooltip label="List view">
							<TopBarButton
								group
								left
								active={layoutMode === 'list'}
								icon={Rows}
								onClick={() => set({ layoutMode: 'list' })}
							/>
						</Tooltip>
						<Tooltip label="Grid view">
							<TopBarButton
								group
								right
								active={layoutMode === 'grid'}
								icon={SquaresFour}
								onClick={() => set({ layoutMode: 'grid' })}
							/>
						</Tooltip>
					</div>
					<SearchBar ref={searchRef} />

					<div className="flex mx-8 space-x-2">
						<Tooltip label="Major Key Alert">
							<TopBarButton icon={Key} />
						</Tooltip>
						{/* <Tooltip label="Cloud">
							<TopBarButton icon={Cloud} />
						</Tooltip> */}
						<Tooltip label="Generate Thumbnails">
							<TopBarButton
								icon={ArrowsClockwise}
								onClick={() => {
									// generateThumbsForLocation({ id: locationId, path: '' });
								}}
							/>
						</Tooltip>
					</div>
				</div>
				<div className="flex mr-3 space-x-2">
					<Dropdown
						// className="absolute block h-6 w-44 top-2 right-4"
						align="right"
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
			</div>
		</>
	);
};
