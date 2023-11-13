/**
 * rehype-image-size.js
 *
 * Requires:
 * - image-size
 * - unist-util-visit
 */
import getImageSize from 'image-size';
import { visit } from 'unist-util-visit';

/**
 * Analyze local MDX images and add `width` and `height` attributes to the
 * generated `img` elements.
 * Supports both markdown-style images and MDX <Image /> components.
 * @param {string} options.root - The root path when reading the image file.
 */
export const rehypeImageSize = (options) => {
	return (tree) => {
		// This matches all images that use the markdown standard format ![label](path).
		visit(tree, { type: 'element', tagName: 'img' }, (node) => {
			if (node.properties.width || node.properties.height) {
				return;
			}
			const imagePath = `${options?.root ?? ''}${node.properties.src}`;
			const imageSize = getImageSize(imagePath);
			node.properties.width = imageSize.width;
			node.properties.height = imageSize.height;
		});
		// This matches all MDX' <Image /> components.
		visit(tree, { type: 'mdxJsxFlowElement', name: 'Image' }, (node) => {
			const srcAttr = node.attributes?.find((attr) => attr.name === 'src');
			const imagePath = `${options?.root ?? ''}${srcAttr.value}`;
			const imageSize = getImageSize(imagePath);
			const widthAttr = node.attributes?.find((attr) => attr.name === 'width');
			const heightAttr = node.attributes?.find((attr) => attr.name === 'height');
			if (widthAttr || heightAttr) {
				// If `width` or `height` have already been set explicitly we
				// don't want to override them.
				return;
			}
			node.attributes.push({
				type: 'mdxJsxAttribute',
				name: 'width',
				value: imageSize.width
			});
			node.attributes.push({
				type: 'mdxJsxAttribute',
				name: 'height',
				value: imageSize.height
			});
		});
	};
};

export default rehypeImageSize;
