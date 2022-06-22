import clsx from 'clsx';
import { FilePlus, FileText, Plus, Share, Trash } from 'phosphor-react';
import React, { MouseEventHandler } from 'react';

import icons from '../../assets/icons';
import { ReactComponent as Folder } from '../../assets/svg/folder.svg';
import { WithContextMenu } from '../layout/MenuOverlay';
import { DefaultProps } from '../primitive/types';

interface Props extends DefaultProps {
	fileName: string;
	iconName?: string;
	format?: string;
	folder?: boolean;
	selected?: boolean;
	onClick?: MouseEventHandler<HTMLDivElement>;
}

export default function FileItem(props: Props) {
	// const Shadow = () => {
	//   return (
	//     <div
	//       className={clsx(
	//         'absolute opacity-100 transition-opacity duration-200 top-auto bottom-auto w-[64px] h-[40px] shadow-xl shadow-red-500',
	//         { 'opacity-100': props.selected }
	//       )}
	//     />
	//   );
	// };

	return (
		<WithContextMenu
			menu={[
				[
					{
						label: 'Details',
						icon: FileText,
						onClick() {}
					},
					{
						label: 'Share',
						icon: Share,
						onClick() {
							navigator.share?.({
								title: 'Spacedrive',
								text: 'Check out this cool app',
								url: 'https://spacedrive.com'
							});
						}
					}
				],
				[
					{
						label: 'More actions...',
						icon: Plus,
						onClick() {},
						children: [
							[
								{
									label: 'Move to library',
									icon: FilePlus,
									onClick() {}
								}
							]
						]
					}
				],
				[
					{
						label: 'Delete',
						icon: Trash,
						danger: true,
						onClick() {}
					}
				]
			]}
		>
			<div onClick={props.onClick} className="inline-block w-[100px] mb-3" draggable>
				<div
					className={clsx(
						'border-2 border-transparent rounded-lg text-center w-[100px] h-[100px] mb-1',
						{
							'bg-gray-50 dark:bg-gray-650': props.selected
						}
					)}
				>
					{props.folder ? (
						<div className="flex items-center justify-center w-full h-full active:translate-y-[1px]">
							<div className="w-[70px]">
								<Folder className="" />
							</div>
						</div>
					) : (
						<div
							className={clsx(
								'w-[64px] mt-1.5 m-auto transition duration-200 rounded-lg h-[90px] relative active:translate-y-[1px]',
								{
									'': props.selected
								}
							)}
						>
							<svg
								className="absolute top-0 left-0 pointer-events-none fill-gray-150 dark:fill-gray-550"
								width="65"
								height="85"
								viewBox="0 0 65 81"
							>
								<path d="M0 8C0 3.58172 3.58172 0 8 0H39.6863C41.808 0 43.8429 0.842855 45.3431 2.34315L53.5 10.5L62.6569 19.6569C64.1571 21.1571 65 23.192 65 25.3137V73C65 77.4183 61.4183 81 57 81H8C3.58172 81 0 77.4183 0 73V8Z" />
							</svg>
							<svg
								width="22"
								height="22"
								className="absolute top-1 -right-[1px] z-10 fill-gray-50 dark:fill-gray-500 pointer-events-none"
								viewBox="0 0 41 41"
							>
								<path d="M41.4116 40.5577H11.234C5.02962 40.5577 0 35.5281 0 29.3238V0L41.4116 40.5577Z" />
							</svg>
							<div className="absolute flex flex-col items-center justify-center w-full h-full">
								{props.iconName && icons[props.iconName as keyof typeof icons] ? (
									(() => {
										const Icon = icons[props.iconName as keyof typeof icons];
										return (
											<Icon className="mt-2 pointer-events-none margin-auto w-[40px] h-[40px]" />
										);
									})()
								) : (
									<></>
								)}
								<span className="mt-1 text-xs font-bold text-center uppercase cursor-default text-gray-450">
									{props.format}
								</span>
							</div>
						</div>
					)}
				</div>
				<div className="flex justify-center">
					<span
						className={clsx(
							'px-1.5 py-[1px] rounded-md text-sm font-medium text-gray-550 dark:text-gray-300 cursor-default',
							{
								'bg-primary !text-white': props.selected
							}
						)}
					>
						{props.fileName}
					</span>
				</div>
			</div>
		</WithContextMenu>
	);
}
