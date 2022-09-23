import { Tag } from '@tryghost/content-api';

export interface BlogTagProps {
	tag: Tag;
}

export const BlogTag = (props: BlogTagProps) => {
	return (
		<span
			className={`px-2 py-0.5 rounded-md text-gray-500 text-sm  bg-gray-550`}
			style={{
				backgroundColor: props.tag.accent_color + '' ?? '',
				color: parseInt(props.tag.accent_color?.slice(1) ?? '', 16) > 0xffffff / 2 ? '#000' : '#fff'
			}}
		>
			{props.tag.name}
		</span>
	);
};
