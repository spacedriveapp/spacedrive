import { MDXComponents } from 'mdx/types';
import NextImage, { ImageProps } from 'next/image';

import Notice from './Notice';

const Image = (props: ImageProps) => <NextImage {...props} />;

export const BlogMDXComponents = {
	img: Image, // we remap 'img' to 'Image'
	Image
} as MDXComponents;

export const DocMDXComponents = { img: Image, Image, Notice } as MDXComponents;
