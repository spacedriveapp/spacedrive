import { CircleDashed, Folder, Icon, Tag } from '@phosphor-icons/react';
import { IconTypes } from '@sd/assets/util';
import clsx from 'clsx';
import { InOrNotIn, MaybeNot, OptionalRange, TextMatch } from '@sd/client';
import { Icon as SDIcon } from '~/components';

function isIn<T>(kind: InOrNotIn<T>): kind is { in: T[] } {
	return 'in' in kind;
}

export function inOrNotIn<T>(
	kind: InOrNotIn<T> | null | undefined,
	value: T,
	condition: boolean
): InOrNotIn<T> {
	if (condition) {
		if (kind && isIn(kind)) {
			kind.in.push(value);
			return kind;
		} else {
			return { in: [value] };
		}
	} else {
		if (kind && !isIn(kind)) {
			kind.notIn.push(value);
			return kind;
		} else {
			return { notIn: [value] };
		}
	}
}

export const maybeNot = <T,>(value: T, condition: boolean): MaybeNot<T> => {
	return condition ? value : { not: value };
};

export function textMatch(type: 'contains' | 'startsWith' | 'endsWith' | 'equals') {
	return (value: string): TextMatch => {
		switch (type) {
			case 'contains':
				return { contains: value };
			case 'startsWith':
				return { startsWith: value };
			case 'endsWith':
				return { endsWith: value };
			case 'equals':
				return { equals: value };
			default:
				throw new Error('Invalid TextMatch type.');
		}
	};
}

export function optionalRange<T>(from: T, to: T): OptionalRange<T> {
	return { from, to };
}

// this could be handy elsewhere
export const RenderIcon = ({
	className,
	icon
}: {
	icon?: Icon | IconTypes | string;
	className?: string;
}) => {
	if (typeof icon === 'string' && icon.startsWith('#')) {
		return (
			<div
				className={clsx('mr-0.5 h-[15px] w-[15px] shrink-0 rounded-full border', className)}
				style={{
					backgroundColor: icon ? icon : 'transparent',
					borderColor: icon || '#efefef'
				}}
			/>
		);
	} else if (typeof icon === 'string') {
		return (
			<SDIcon
				name={icon as any}
				size={20}
				className={clsx('shrink-0 text-ink-dull', className)}
			/>
		);
	} else {
		const IconComponent = icon;
		return (
			IconComponent && (
				<IconComponent
					size={16}
					weight="bold"
					className={clsx('shrink-0 text-ink-dull group-hover:text-white', className)}
				/>
			)
		);
	}
};

export const getIconComponent = (iconName: string): Icon => {
	const icons: Record<string, React.ComponentType> = {
		Folder,
		CircleDashed,
		Tag
	};

	return icons[iconName] as Icon;
};
