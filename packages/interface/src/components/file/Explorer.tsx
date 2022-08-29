import { ExplorerKind, rspc, useExplorerStore, useLibraryQuery, useLibraryStore } from '@sd/client';
import { DirectoryWithContents } from '@sd/core';
import { ContextMenu } from '@sd/ui';
import {
	ArrowBendUpRight,
	FilePlus,
	FileText,
	FileX,
	FileZip,
	LockSimple,
	Package,
	Plus,
	Share,
	TagSimple,
	Trash,
	TrashSimple
} from 'phosphor-react';
import React from 'react';
import { useParams, useSearchParams } from 'react-router-dom';

import { FileList } from '../file/FileList';
import { Inspector } from '../file/Inspector';
import { WithContextMenu } from '../layout/MenuOverlay';
import { TopBar } from '../layout/TopBar';

interface Props {
	library_id: string;
	kind: ExplorerKind;
	identifier: number;
	files?: DirectoryWithContents;
	heading?: string;
}

export default function Explorer(props: Props) {
	const { selectedRowIndex, addNewThumbnail, contextMenuObjectId } = useExplorerStore();

	const { data: tags } = useLibraryQuery(['tags.getAll'], {});

	rspc.useSubscription(['jobs.newThumbnail', { library_id: props.library_id!, arg: null }], {
		onNext: (cas_id) => {
			addNewThumbnail(cas_id);
		}
	});

	return (
		<div className="relative">
			<WithContextMenu
				menu={[
					[
						{
							label: 'Open',
							onClick() {}
						},
						{
							label: 'Open with...',
							onClick() {}
						}
					],
					[
						{
							label: 'Quick view',
							onClick() {}
						},
						{
							label: 'Open in Finder',
							onClick() {}
						}
					],
					[
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
							label: 'Assign tag',
							icon: TagSimple,
							onClick() {},
							children: [
								tags?.map((tag) => ({
									label: tag.name || ''
								})) || []
							]
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
								],
								[
									{
										label: 'Encrypt',
										icon: LockSimple,
										onClick() {}
									},
									{
										label: 'Compress',
										icon: Package,
										onClick() {}
									},
									{
										label: 'Convert to',
										icon: ArrowBendUpRight,
										onClick() {},
										children: [
											[
												{
													label: 'PNG',
													onClick() {}
												},
												{
													label: 'WebP',
													onClick() {}
												}
											]
										]
									}
								],
								[
									{
										label: 'Remove from library',
										icon: FileX,
										onClick() {}
									}
								],
								[
									{
										label: 'Secure delete',
										icon: TrashSimple,
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
				<div className="relative flex flex-col w-full bg-gray-650">
					<TopBar />
					<div className="relative flex flex-row w-full max-h-full">
						<FileList
							location_id={props.identifier}
							files={props.files?.contents || []}
							heading={props.files?.directory.name || props.heading}
						/>
						{props.files?.contents && (
							<Inspector
								locationId={props.identifier}
								selectedFile={props.files.contents[selectedRowIndex]}
							/>
						)}
					</div>
				</div>
			</WithContextMenu>
		</div>
	);
}
