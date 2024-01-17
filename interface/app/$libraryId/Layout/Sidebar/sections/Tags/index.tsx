import clsx from 'clsx';
import { NavLink, useMatch } from 'react-router-dom';
import { useCache, useLibraryQuery, useNodes, type Tag } from '@sd/client';
import { useExplorerDroppable } from '~/app/$libraryId/Explorer/useExplorerDroppable';
import { SubtleButton } from '~/components';

import SidebarLink from '../../SidebarLayout/Link';
import Section from '../../SidebarLayout/Section';
import { SeeMore } from '../../SidebarLayout/SeeMore';
import { ContextMenu } from './ContextMenu';

export default function TagsSection() {
	const result = useLibraryQuery(['tags.list'], { keepPreviousData: true });
	useNodes(result.data?.nodes);
	const tags = useCache(result.data?.items);

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
			<SeeMore>
				{tags.map((tag) => (
					<Tag key={tag.id} tag={tag} />
				))}
			</SeeMore>
		</Section>
	);
}

const Tag = ({ tag }: { tag: Tag }) => {
	const tagId = useMatch('/:libraryId/tag/:tagId')?.params.tagId;

	const { isDroppable, className, setDroppableRef } = useExplorerDroppable({
		id: `sidebar-tag-${tag.id}`,
		allow: ['Path', 'Object'],
		data: { type: 'tag', data: tag },
		navigateTo: `tag/${tag.id}`,
		disabled: Number(tagId) === tag.id
	});

	return (
		<ContextMenu key={tag.id} tagId={tag.id}>
			<SidebarLink
				ref={setDroppableRef}
				to={`tag/${tag.id}`}
				className={clsx(
					'border radix-state-open:border-accent',
					isDroppable ? ' border-accent' : 'border-transparent',
					className
				)}
			>
				<div
					className="size-[12px] shrink-0 rounded-full"
					style={{ backgroundColor: tag.color || '#efefef' }}
				/>
				<span className="ml-1.5 truncate text-sm">{tag.name}</span>
			</SidebarLink>
		</ContextMenu>
	);
};
