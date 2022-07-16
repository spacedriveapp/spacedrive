import React from 'react';

import Card from '../../../components/layout/Card';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

const tags = [
	{ id: 1, name: 'Keepsafe', color: '#FF6788' },
	{ id: 2, name: 'OBS', color: '#BF88FF' },
	{ id: 3, name: 'BlackMagic', color: '#F0C94A' },
	{ id: 4, name: 'Camera Roll', color: '#00F0DB' },
	{ id: 5, name: 'Spacedrive', color: '#00F079' }
];

export default function TagsSettings() {
	return (
		<SettingsContainer>
			<SettingsHeader title="Tags" description="Manage your tags." />

			<Card className="!px-2 dark:bg-gray-800">
				<div className="flex flex-wrap gap-2">
					{tags.map((tag) => (
						<div
							key={tag.id}
							className="flex items-center rounded px-1.5 py-0.5"
							style={{ backgroundColor: tag.color + 'CC' }}
						>
							<span className="text-xs text-white drop-shadow-md">{tag.name}</span>
						</div>
					))}
				</div>
			</Card>
		</SettingsContainer>
	);
}
