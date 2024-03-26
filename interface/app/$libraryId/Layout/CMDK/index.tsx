import './CMDK.css';
import './CMDK.scss';

import clsx from 'clsx';
import { useEffect, useState } from 'react';
import CommandPalette, { filterItems, getItemIndex } from 'react-cmdk';
import { useNavigate } from 'react-router';
import { createSearchParams } from 'react-router-dom';
import {
	arraysEqual,
	useCache,
	useLibraryContext,
	useLibraryQuery,
	useNodes,
	useOnlineLocations
} from '@sd/client';
import { dialogManager } from '@sd/ui';
import i18n from '~/app/I18n';
import { Icon } from '~/components';
import Sparkles from '~/components/Sparkles';
import { useShortcut } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { explorerStore } from '../../Explorer/store';
import { AddLocationDialog } from '../../settings/library/locations/AddLocationDialog';
import { openDirectoryPickerDialog } from '../../settings/library/locations/openDirectoryPickerDialog';
import CreateDialog from '../../settings/library/tags/CreateDialog';
import CMDKLocations from './pages/CMDKLocations';
import CMDKTags from './pages/CMDKTags';

const CMDK = () => {
	const [isOpen, setIsOpen] = useState<boolean>(false);

	const platform = usePlatform();
	const libraryId = useLibraryContext().library.uuid;

	useShortcut('toggleCommandPalette', (e) => {
		e.preventDefault();
		e.stopPropagation();
		setIsOpen((v) => !v);
	});

	useShortcut('closeCommandPalette', (e) => {
		e.preventDefault();
		e.stopPropagation();
		if (isOpen) {
			setIsOpen(false);
		}
	});

	useEffect(() => {
		explorerStore.isCMDPOpen = isOpen;
	}, [isOpen]);

	const [page, setPage] = useState<'root' | 'locations' | 'tags'>('root');
	const [search, setSearch] = useState('');

	const locationsQuery = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	useNodes(locationsQuery.data?.nodes);
	const locations = useCache(locationsQuery.data?.items);

	const onlineLocations = useOnlineLocations();

	function handleClose(open: boolean) {
		setIsOpen(open);
		// Reset page and search
		setPage('root');
		setSearch('');
	}

	const navigate = useNavigate();

	const filteredItems = filterItems(
		[
			{
				heading: i18n.t('coming_soon'),
				id: 'top',
				items: [
					{
						id: 'ask-spacedrive',
						children: (
							<Sparkles>
								<span>Ask Spacedrive</span>
							</Sparkles>
						),
						icon: 'SparklesIcon',
						closeOnSelect: false,
						disabled: true // Disabled for now
					}
				]
			},
			// Navigation
			{
				heading: 'Navigation',
				id: 'navigation',
				items: [
					{
						id: 'go-settings',
						children: 'Go to settings',
						icon: 'ArrowRightIcon',
						closeOnSelect: true,
						onClick: () => navigate('settings/client/general')
					},
					{
						id: 'go-overview',
						children: 'Go to overview',
						icon: 'ArrowRightIcon',
						closeOnSelect: true,
						onClick: () => navigate('overview')
					},
					{
						id: 'go-recents',
						children: 'Go to recents',
						icon: 'ArrowRightIcon',
						closeOnSelect: true,
						onClick: () => navigate('recents')
					},
					{
						id: 'go-labels',
						children: 'Go to labels',
						icon: 'ArrowRightIcon',
						closeOnSelect: true,
						onClick: () => navigate('labels')
					},
					{
						id: 'go-location',
						children: 'Go to location',
						icon: 'ArrowRightIcon',
						closeOnSelect: false,
						onClick: () => setPage('locations')
					},
					{
						id: 'go-tag',
						children: 'Go to tag',
						icon: 'ArrowRightIcon',
						closeOnSelect: false,
						onClick: () => setPage('tags')
					}

					// {
					// 	id: 'go-to-settings',
					// 	children: 'Go to settings',
					// 	icon: 'SettingsIcon',
					// 	onClick: () => {}
					// }
				]
			},
			{
				heading: 'Actions',
				id: 'actions',
				items: [
					// {
					// 	id: 'create-folder',
					// 	children: 'Create folder',
					// 	icon: 'FolderPlusIcon',
					// 	onClick: () => {}
					// },
					{
						id: 'create-tag',
						children: 'Create tag',
						icon: 'TagIcon',
						onClick: () => {
							dialogManager.create((dp) => <CreateDialog {...dp} />);
						}
					},
					{
						id: 'add-location',
						children: 'Add location',
						icon: 'FolderIcon',
						onClick: async () => {
							const path = await openDirectoryPickerDialog(platform);
							if (path !== '') {
								dialogManager.create((dp) => (
									<AddLocationDialog
										path={path ?? ''}
										libraryId={libraryId}
										{...dp}
									/>
								));
							}
						}
					}
				]
			},
			// TODO: Might look nice if we showed some items and maybe saved searches here
			// {
			// 	heading: `Searching for "${search}"`,
			// 	id: 'search',
			// 	items: [
			// 		// objects.items
			// 		// 	? (objects.items.map((object, index) => {
			// 		// 			const item = isPath(object);
			// 		// 			return {
			// 		// 				id: index,
			// 		// 				children: isPath(object) && object.item.name,
			// 		// 				icon: () => (
			// 		// 					<div className="relative -mt-0.5 mr-1 shrink-0 grow-0">
			// 		// 						<Icon name="Location" size={22} />
			// 		// 					</div>
			// 		// 				)
			// 		// 			};
			// 		// 		}) as any)
			// 		// 	: ([] as any)
			// 	]
			// }
			// This is technically a duplicate of "Go to Location", but it looks cool.
			{
				heading: 'Locations',
				id: 'locations',
				items: locations
					? locations.map((location) => ({
							id: location.id,
							children: location.name,
							icon: () => (
								<div className="relative -mt-0.5 mr-1 shrink-0 grow-0">
									<Icon name="Folder" size={22} />
									<div
										className={clsx(
											'absolute bottom-0.5 right-0 size-1.5 rounded-full',
											onlineLocations.some((l) =>
												arraysEqual(location.pub_id, l)
											)
												? 'bg-green-500'
												: 'bg-red-500'
										)}
									/>
								</div>
							),
							onClick: () => navigate(`location/${location.id}`)
						}))
					: ([] as any)
			}
		],
		search
	);

	return (
		<CommandPalette
			onChangeSearch={setSearch}
			onChangeOpen={handleClose}
			search={search}
			isOpen={isOpen}
			page={page}
			placeholder="Search for files and actions..."
			// footer
		>
			<CommandPalette.Page id="root" onEscape={() => setSearch('')}>
				{filteredItems.length ? (
					filteredItems.map((list) => (
						<CommandPalette.List key={list.id} heading={list.heading}>
							{list.items.map(({ id, ...rest }) => (
								<CommandPalette.ListItem
									key={id}
									index={getItemIndex(filteredItems, id)}
									{...rest}
								/>
							))}
						</CommandPalette.List>
					))
				) : (
					<CommandPalette.FreeSearchAction
						onClick={(v) =>
							navigate(
								{
									pathname: 'search',
									search: createSearchParams({ search }).toString()
								},
								{ replace: true }
							)
						}
					/>
				)}
			</CommandPalette.Page>
			{page === 'locations' && <CMDKLocations />}
			{page === 'tags' && <CMDKTags />}
		</CommandPalette>
	);
};

export default CMDK;
