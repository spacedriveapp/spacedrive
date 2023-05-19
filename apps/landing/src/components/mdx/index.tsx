import NextImage, { ImageProps } from 'next/image';
import Notice from './Notice';

const MDXImage = (props: ImageProps) => <NextImage {...props} />;

export const BlogMDXComponents = { MDXImage };
export const DocMDXComponents = { MDXImage, Notice };
