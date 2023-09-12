import NextImage, { ImageProps } from 'next/image';

import Notice from './Notice';

const Image = (props: ImageProps) => <NextImage {...props} />;

export const BlogMDXComponents = { Image };
export const DocMDXComponents = { Image, Notice };
