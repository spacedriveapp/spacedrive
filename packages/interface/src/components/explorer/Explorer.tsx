import {
	rspc,
	useExplorerStore,
	useLibraryMutation,
	useLibraryQuery,
	useLibraryStore
} from '@sd/client';
import { ExplorerData } from '@sd/core';
import {
	ArrowBendUpRight,
	LockSimple,
	Package,
	Plus,
	Share,
	TagSimple,
	Trash,
	TrashSimple
} from 'phosphor-react';
import React from 'react';

import { FileList } from '../explorer/FileList';
import { Inspector } from '../explorer/Inspector';
import { WithContextMenu } from '../layout/MenuOverlay';
import { TopBar } from '../layout/TopBar';

interface Props {
	data: ExplorerData;
}

export default function Explorer(props: Props) {
	const { selectedRowIndex, addNewThumbnail, contextMenuObjectId, showInspector } =
		useExplorerStore();

	const { currentLibraryUuid } = useLibraryStore();

	const { data: tags } = useLibraryQuery(['tags.getAll'], {});

	const { mutate: assignTag } = useLibraryMutation('tags.assign');

	const { data: tagsForFile } = useLibraryQuery(['tags.getForFile', contextMenuObjectId || -1]);

	rspc.useSubscription(['jobs.newThumbnail', { library_id: currentLibraryUuid!, arg: null }], {
		onNext: (cas_id) => {
			addNewThumbnail(cas_id);
		}
	});

	return (
		<div className="relative">
			<WithContextMenu
				menu={[
					[
						// `file-${props.identifier}`,
						{
							label: 'Open'
						},
						{
							label: 'Open with...'
						}
					],
					[
						{
							label: 'Quick view'
						},
						{
							label: 'Open in Finder'
						}
					],
					[
						{
							label: 'Rename'
						},
						{
							label: 'Duplicate'
						}
					],
					[
						{
							label: 'Share',
							icon: Share,
							onClick(e) {
								e.preventDefault();
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
							children: [
								tags?.map((tag) => {
									const active = !!tagsForFile?.find((t) => t.id === tag.id);
									return {
										label: tag.name || '',

										// leftItem: <Checkbox checked={!!tagsForFile?.find((t) => t.id === tag.id)} />,
										leftItem: (
											<div className="relative">
												<div
													className="block w-[15px] h-[15px] mr-0.5 border rounded-full"
													style={{
														backgroundColor: active
															? tag.color || '#efefef'
															: 'transparent' || '#efefef',
														borderColor: tag.color || '#efefef'
													}}
												/>
											</div>
										),
										onClick(e) {
											e.preventDefault();
											if (contextMenuObjectId != null)
												assignTag({
													tag_id: tag.id,
													file_id: contextMenuObjectId,
													unassign: active
												});
										}
									};
								}) || []
							]
						}
					],
					[
						{
							label: 'More actions...',
							icon: Plus,

							children: [
								// [
								// 	{
								// 		label: 'Move to library',
								// 		icon: FilePlus,
								// 		children: [libraries?.map((library) => ({ label: library.config.name })) || []]
								// 	},
								// 	{
								// 		label: 'Remove from library',
								// 		icon: FileX
								// 	}
								// ],
								[
									{
										label: 'Encrypt',
										icon: LockSimple
									},
									{
										label: 'Compress',
										icon: Package
									},
									{
										label: 'Convert to',
										icon: ArrowBendUpRight,

										children: [
											[
												{
													label: 'PNG'
												},
												{
													label: 'WebP'
												}
											]
										]
									}
									// {
									// 	label: 'Mint NFT',
									// 	icon: TrashIcon
									// }
								],
								[
									{
										label: 'Secure delete',
										icon: TrashSimple
									}
								]
							]
						}
					],
					[
						{
							label: 'Delete',
							icon: Trash,
							danger: true
						}
					]
				]}
			>
				<div className="relative flex flex-col w-full bg-gray-650">
					<TopBar />
					<div className="relative flex flex-row w-full max-h-full">
						<FileList data={props.data?.items || []} context={props.data.context} />
						{showInspector && (
							<div className="min-w-[260px] max-w-[260px]">
								{props.data.items[selectedRowIndex]?.id && (
									<Inspector
										key={props.data.items[selectedRowIndex].id}
										data={props.data.items[selectedRowIndex]}
									/>
								)}
							</div>
						)}
					</div>
				</div>
			</WithContextMenu>
		</div>
	);
}
