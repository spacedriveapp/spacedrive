import { SearchIcon } from '@heroicons/react/solid';
import { Button, Input } from '@sd/ui';
import React from 'react';

import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

// extensions should cache their logos in the app data folder
interface ExtensionItemData {
	name: string;
	uuid: string;
	platforms: ['windows' | 'macOS' | 'linux'];
	installed: boolean;
	description: string;
	logoUri: string;
}

const extensions: ExtensionItemData[] = [
	{
		name: 'Apple Photos',
		uuid: 'com.apple.photos',
		installed: true,
		platforms: ['macOS'],
		description: 'Import photos and videos with metadata from Apple Photos.',
		logoUri: 'https://apple.com/apple-logo.png'
	},
	{
		name: 'Twitch VOD Archiver',
		uuid: 'com.apple.photos',
		installed: false,
		platforms: ['macOS'],
		description: 'Apple Photos is a photo management application for Mac.',
		logoUri: 'https://apple.com/apple-logo.png'
	},
	{
		name: 'Shared Clipboard',
		uuid: 'com.apple.photos',
		installed: false,
		platforms: ['macOS'],
		description: 'Apple Photos is a photo management application for Mac.',
		logoUri: 'https://apple.com/apple-logo.png'
	}
];

function ExtensionItem(props: { extension: ExtensionItemData }) {
	const { installed, name, description } = props.extension;

	return (
		<div className="flex flex-col w-[290px] px-4 py-4 bg-gray-600 border border-gray-500 rounded">
			<h3 className="m-0 text-sm font-bold">{name}</h3>
			<p className="mt-1 mb-1 text-xs text-gray-300 ">{description}</p>
			<Button size="sm" className="mt-2" variant={installed ? 'gray' : 'primary'}>
				{installed ? 'Installed' : 'Install'}
			</Button>
		</div>
	);
}

export default function ExtensionSettings() {
	// const { data: volumes } = useBridgeQuery('GetVolumes');

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Extensions"
				description="Install extensions to extend the functionality of this client."
				rightArea={
					<div className="relative mt-6">
						<SearchIcon className="absolute w-[18px] h-auto top-[8px] left-[11px] text-gray-350" />
						<Input className="w-56 !p-0.5 !pl-9" placeholder="Search extensions" />
					</div>
				}
			/>

			<div className="flex flex-wrap gap-3">
				{extensions.map((extension) => (
					<ExtensionItem key={extension.uuid} extension={extension} />
				))}
			</div>
		</SettingsContainer>
	);
}
