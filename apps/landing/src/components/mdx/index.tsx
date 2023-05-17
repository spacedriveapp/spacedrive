import NextImage, { ImageProps } from 'next/image';

const MDXImage = (props: ImageProps) => <NextImage {...props} />;

export const BlogMDXComponents = { MDXImage };
export const DocMDXComponents = { MDXImage };
