import { keepPreviousData } from '@tanstack/react-query';
import clsx from 'clsx';
import CommandPalette from 'react-cmdk';
import { useNavigate } from 'react-router';

import { arraysEqual, useLibraryQuery, useOnlineLocations } from '@sd/client';
import { Icon } from '~/components';

export default function CMDKLocations() {
	const locationsQuery = useLibraryQuery(['locations.list'], {
		placeholderData: keepPreviousData
	});
	const locations = locationsQuery.data;

	const onlineLocations = useOnlineLocations();

	const navigate = useNavigate();

	return (
		<CommandPalette.Page id="locations">
			<CommandPalette.List>
				{locations?.map((location, index) => (
					<CommandPalette.ListItem
						key={location.id}
						index={index}
						onClick={() => navigate(`location/${location.id}`)}
						closeOnSelect
					>
						<div className="relative mr-1 shrink-0 grow-0">
							<Icon name="Folder" size={18} />
							<div
								className={clsx(
									'absolute bottom-0.5 right-0 size-1.5 rounded-full',
									onlineLocations.some(l => arraysEqual(location.pub_id, l))
										? 'bg-green-500'
										: 'bg-red-500'
								)}
							/>
						</div>

						<span className="truncate">{location.name}</span>
					</CommandPalette.ListItem>
				))}
			</CommandPalette.List>
		</CommandPalette.Page>
	);
}
