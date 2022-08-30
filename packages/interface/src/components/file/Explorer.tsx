import {
	ExplorerKind,
	rspc,
	useBridgeQuery,
	useCurrentLibrary,
	useExplorerStore,
	useLibraryMutation,
	useLibraryQuery,
	useLibraryStore
} from '@sd/client';
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

	const { mutate: assignTag } = useLibraryMutation('tags.assign', {
		onSettled: () => {
			console.log('assigned tag', tags, contextMenuObjectId);
		},
		onError: (error) => {
			console.error(error);
		}
	});

	const { currentLibrary, libraries, currentLibraryUuid } = useCurrentLibrary();

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

							children: [
								tags?.map((tag) => ({
									label: tag.name || '',
									onClick() {
										assignTag({
											tag_id: tag.id,
											file_id: contextMenuObjectId
										});
									}
								})) || []
							]
						}
					],
					[
						{
							label: 'More actions...',
							icon: Plus,

							children: [
								[
									{
										label: 'Move to library',
										icon: FilePlus,
										children: [libraries?.map((library) => ({ label: library.config.name })) || []]
									},
									{
										label: 'Remove from library',
										icon: FileX
									}
								],
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
				<div className="relative flex flex-col w-full bg-gray-600">
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
