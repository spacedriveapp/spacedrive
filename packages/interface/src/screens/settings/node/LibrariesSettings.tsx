import { CollectionIcon } from '@heroicons/react/outline';
import { PlusIcon } from '@heroicons/react/solid';
import { useBridgeCommand, useBridgeQuery } from '@sd/client';
import { LibraryConfig } from '@sd/core';
import { Button } from '@sd/ui';
import React, { useContext } from 'react';

import { AppPropsContext } from '../../../AppPropsContext';
import Card from '../../../components/layout/Card';
import { Toggle } from '../../../components/primitive';
import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

// type LibrarySecurity = 'public' | 'password' | 'vault';

function LibraryListItem(props: { library: LibraryConfig }) {
	return (
		<Card>
			<h3 className="font-semibold">{props.library.name}</h3>
		</Card>
	);
}

export default function LibrarySettings() {
	// const locations = useBridgeQuery("SysGetLocation")
	const { data: libraries } = useBridgeQuery('NodeGetLibraries');

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Libraries"
				description="The database contains all library data and file metadata."
				rightArea={
					<div className="flex-row space-x-2">
						<Button variant="primary" size="sm">
							Add Library
						</Button>
					</div>
				}
			/>

			<div>
				{libraries?.map((library) => (
					<LibraryListItem key={library.name} library={library} />
				))}
			</div>
		</SettingsContainer>
	);
}
