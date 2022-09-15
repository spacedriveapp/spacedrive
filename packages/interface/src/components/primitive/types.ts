import { DetailedHTMLProps, HTMLAttributes } from 'react';

export interface DefaultProps<E extends HTMLElement = HTMLElement>
	extends DetailedHTMLProps<HTMLAttributes<E>, E> {
	className?: string;
}
