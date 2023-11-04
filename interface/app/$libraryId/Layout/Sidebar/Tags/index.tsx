import { NavLink } from 'react-router-dom';
import { useLibraryQuery } from '@sd/client';
import { SubtleButton } from '~/components';

import SidebarLink from '../Link';
import Section from '../Section';
import SeeMore from '../SeeMore';
import { ContextMenu } from './ContextMenu';

export const Tags = () => {
	const { data: tags } = useLibraryQuery(['tags.list'], { keepPreviousData: true });

	if (!tags?.length) return null;

	return (
		<Section
			name="Tags"
			actionArea={
				<NavLink to="settings/library/tags">
					<SubtleButton />
				</NavLink>
			}
		>
			<SeeMore
				items={tags}
				renderItem={(tag) => (
					<ContextMenu key={tag.id} tagId={tag.id}>
						<SidebarLink
							to={`tag/${tag.id}`}
							className="border border-transparent radix-state-open:border-accent"
						>
							<div
								className="h-[12px] w-[12px] shrink-0 rounded-full"
								style={{ backgroundColor: tag.color || '#efefef' }}
							/>
							<span className="ml-1.5 truncate text-sm">{tag.name}</span>
						</SidebarLink>
					</ContextMenu>
				)}
			/>
		</Section>
	);
};
