import clsx from 'clsx';
import { useEffect, useState } from 'react';
import { useLoaderData } from 'react-router';
import { Tag, useLibraryQuery } from '@sd/client';
import { Button, Card, dialogManager } from '@sd/ui';
import { Heading } from '~/app/$libraryId/settings/Layout';
import type { LocationIdParams } from '~/app/$libraryId/settings/library';
import CreateDialog from './CreateDialog';
import EditForm from './EditForm';

export const Component = () => {
	const tags = useLibraryQuery(['tags.list']);
	const { id: locationId } = useLoaderData() as LocationIdParams;
	const tagSelectedParam = tags.data?.find((tag) => tag.id === locationId);
	const [selectedTag, setSelectedTag] = useState<null | Tag>(
		tagSelectedParam ?? tags.data?.[0] ?? null
	);

	// Update selected tag when the route param changes
	useEffect(() => {
		setSelectedTag(tagSelectedParam !== undefined ? tagSelectedParam : null);
	}, [tagSelectedParam]);

	return (
		<>
			<Heading
				title="Tags"
				description="Manage your tags."
				rightArea={
					<div className="flex-row space-x-2">
						<Button
							variant="accent"
							size="sm"
							onClick={() => {
								dialogManager.create((dp) => <CreateDialog {...dp} />);
							}}
						>
							Create Tag
						</Button>
					</div>
				}
			/>
			<Card className="!px-2">
				<div className="m-1 flex flex-wrap gap-2">
					{tags.data?.map((tag) => (
						<div
							onClick={() => setSelectedTag(tag.id === selectedTag?.id ? null : tag)}
							key={tag.id}
							className={clsx(
								'flex items-center rounded px-1.5 py-0.5',
								selectedTag?.id === tag.id && 'ring'
							)}
							style={{ backgroundColor: tag.color + 'CC' }}
						>
							<span className="text-xs text-white drop-shadow-md">{tag.name}</span>
						</div>
					))}
				</div>
			</Card>
			{selectedTag ? (
				<EditForm
					key={selectedTag.id}
					tag={selectedTag}
					onDelete={() => setSelectedTag(null)}
				/>
			) : (
				<div className="text-sm font-medium text-gray-400">No Tag Selected</div>
			)}
		</>
	);
};
