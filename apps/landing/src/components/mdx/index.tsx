import { MDXComponents } from 'mdx/types';
import NextImage, { ImageProps } from 'next/image';
import { env } from '~/env';

import Notice from './Notice';
import Video from './Video';
import Pre from './Pre';

const Image = (props: ImageProps) => (
	<NextImage
		// Weirdly enough this works in production but not in dev...
		placeholder={env.NODE_ENV === 'production' ? 'blur' : undefined}
		{...props}
	/>
);

export const BlogMDXComponents = {
	img: Image, // we remap 'img' to 'Image'
	pre: Pre,
	Image,
	Video
} as MDXComponents;

export const DocMDXComponents = { img: Image, Image, Notice, Video, pre: Pre } as MDXComponents;
