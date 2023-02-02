import { Tag } from '@tryghost/content-api';

export interface BlogTagProps {
	tag: Tag;
}

export const BlogTag = (props: BlogTagProps) => {
	return (
		<span
			className={`bg-gray-550 rounded-md px-2 py-0.5 text-sm  text-gray-500`}
			style={{
				backgroundColor: props.tag.accent_color + '' ?? '',
				color: parseInt(props.tag.accent_color?.slice(1) ?? '', 16) > 0xffffff / 2 ? '#000' : '#fff'
			}}
		>
			{props.tag.name}
		</span>
	);
};
