import { MDXComponents } from 'mdx/types';
import NextImage, { ImageProps } from 'next/image';
import { env } from '~/env';

import Notice from './Notice';

const Image = (props: ImageProps) => (
	<NextImage
		// Weirdly enough this works in production but not in dev...
		placeholder={env.NODE_ENV === 'production' ? 'blur' : undefined}
		{...props}
	/>
);

export const BlogMDXComponents = {
	img: Image, // we remap 'img' to 'Image'
	Image
} as MDXComponents;

export const DocMDXComponents = { img: Image, Image, Notice } as MDXComponents;
