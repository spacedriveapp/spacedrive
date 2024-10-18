import { keepPreviousData } from '@tanstack/react-query';
import CommandPalette from 'react-cmdk';
import { useNavigate } from 'react-router';
import { useLibraryQuery, type Tag } from '@sd/client';

export default function CMDKTags() {
	const result = useLibraryQuery(['tags.list'], { placeholderData: keepPreviousData });
	const tags = result.data || [];

	const navigate = useNavigate();

	return (
		<CommandPalette.Page id="tags">
			<CommandPalette.List>
				{tags.map((tag, i) => (
					<CommandPalette.ListItem
						key={tag.id}
						index={i}
						onClick={() => navigate(`tag/${tag.id}`)}
						closeOnSelect={true}
					>
						<div
							className="size-[12px] shrink-0 rounded-full"
							style={{ backgroundColor: tag.color || '#efefef' }}
						/>
						<span className="ml-1.5 truncate text-sm">{tag.name}</span>
					</CommandPalette.ListItem>
				))}
			</CommandPalette.List>
		</CommandPalette.Page>
	);
}
