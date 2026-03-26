import {Markdown as SpaceUIMarkdown} from '@spaceui/ai';

export function Markdown({children, className}: {children: string; className?: string}) {
	return (
		<SpaceUIMarkdown 
			content={children} 
			className={className}
		/>
	);
}
