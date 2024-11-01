'use client';

import clsx, { ClassValue } from 'clsx';
import { CSSProperties, ReactNode, useCallback, useEffect, useRef, useState } from 'react';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

interface MousePosition {
	x: number;
	y: number;
}

function useMousePosition(): MousePosition {
	const [mousePosition, setMousePosition] = useState<MousePosition>({
		x: 0,
		y: 0
	});

	useEffect(() => {
		const handleMouseMove = (event: globalThis.MouseEvent) => {
			setMousePosition({ x: event.clientX, y: event.clientY });
		};

		window.addEventListener('mousemove', handleMouseMove);

		return () => {
			window.removeEventListener('mousemove', handleMouseMove);
		};
	}, []);

	return mousePosition;
}

interface MagicContainerProps {
	children?: ReactNode;
	className?: any;
	style?: CSSProperties;
}

const MagicContainer = ({ children, className, style }: MagicContainerProps) => {
	const containerRef = useRef<HTMLDivElement>(null);
	const mousePosition = useMousePosition();
	const mouse = useRef<{ x: number; y: number }>({ x: 0, y: 0 });
	const containerSize = useRef<{ w: number; h: number }>({ w: 0, h: 0 });
	const [boxes, setBoxes] = useState<Array<HTMLElement>>([]);

	const init = useCallback(() => {
		if (containerRef.current) {
			containerSize.current.w = containerRef.current.offsetWidth;
			containerSize.current.h = containerRef.current.offsetHeight;
		}
	}, []);

	const onMouseMove = useCallback(() => {
		if (containerRef.current) {
			const rect = containerRef.current.getBoundingClientRect();
			const { w, h } = containerSize.current;
			const x = mousePosition.x - rect.left;
			const y = mousePosition.y - rect.top;
			const inside = x < w && x > 0 && y < h && y > 0;

			mouse.current.x = x;
			mouse.current.y = y;
			boxes.forEach((box) => {
				const boxX = -(box.getBoundingClientRect().left - rect.left) + mouse.current.x;
				const boxY = -(box.getBoundingClientRect().top - rect.top) + mouse.current.y;
				box.style.setProperty('--mouse-x', `${boxX}px`);
				box.style.setProperty('--mouse-y', `${boxY}px`);

				if (inside) {
					box.style.setProperty('--opacity', `1`);
				} else {
					box.style.setProperty('--opacity', `0`);
				}
			});
		}
	}, [boxes, mousePosition]);

	useEffect(() => {
		init();
		if (containerRef.current)
			setBoxes(Array.from(containerRef.current.children).map((el) => el as HTMLElement));
	}, [init]);

	useEffect(() => {
		init();
		window.addEventListener('resize', init);

		return () => {
			window.removeEventListener('resize', init);
		};
	}, [setBoxes, init]);

	useEffect(() => {
		onMouseMove();
	}, [mousePosition, onMouseMove]);

	return (
		<div style={style} className={className} ref={containerRef}>
			{children}
		</div>
	);
};

interface MagicCardProps {
	/**
	 * @default div
	 * @type React.ElementType
	 * @description
	 * The component to render the card as
	 * */
	as?: React.ElementType;
	/**
	 * @default ""
	 * @type string
	 * @description
	 * The className of the card
	 */
	className?: string;

	/**
	 * @default ""
	 * @type ReactNode
	 * @description
	 * The children of the card
	 * */
	children?: ReactNode;

	/**
	 * @default 600
	 * @type number
	 * @description
	 * The size of the spotlight effect in pixels
	 * */
	size?: number;

	/**
	 * ]@default "#475569"
	 * @type string
	 * @description
	 * The border color of the card
	 */
	borderColor?: string;

	/**
	 * @default 1
	 * @type number
	 * @description
	 * The border width of the card
	 * */
	borderWidth?: number;

	/**
	 * @default 16
	 * @type number
	 * @description
	 * The border radius of the card
	 * */
	borderRadius?: number;

	/**
	 * @default true
	 * @type boolean
	 * @description
	 * Whether to show the spotlight
	 * */
	spotlight?: boolean;

	/**
	 * @default "rgba(255,255,255,0.03)"
	 * @type string
	 * @description
	 * The color of the spotlight
	 * */
	spotlightColor?: string;

	/**
	 * @default true
	 * @type boolean
	 * @description
	 * Whether to isolate the card which is being hovered
	 * */
	isolated?: boolean;

	/**
	 * @default "transparent"
	 * @type string
	 * @description
	 * The background of the card
	 * */
	background?: string;
}

const MagicCard = ({
	as: Component = 'div',
	className,
	children,
	size = 600,
	borderColor = 'rgba(86,114,157, 0.2)',
	borderWidth = 1,
	borderRadius = 10,
	spotlight = true,
	spotlightColor = 'rgba(86,114,157, 0.01)',
	isolated = false,
	background = 'rgba(255,255,255,0.03)'
}: MagicCardProps) => {
	const spotlightStyles =
		'before:pointer-events-none before:absolute before:w-full before:h-full before:rounded-[var(--border-radius)] before:top-0 before:left-0 before:duration-500 before:transition-opacity before:bg-[radial-gradient(var(--mask-size)_circle_at_var(--mouse-x)_var(--mouse-y),var(--spotlight-color),transparent_40%)] before:z-[3] before:blur-xs';

	const borderStyles =
		'after:pointer-events-none after:absolute after:w-full after:h-full after:rounded-[var(--border-radius)] after:top-0 after:left-0 after:duration-500 after:transition-opacity after:bg-[radial-gradient(var(--mask-size)_circle_at_var(--mouse-x)_var(--mouse-y),var(--border-color),transparent_40%)] after:z-[1]';
	return (
		<Component
			style={{
				'--border-radius': `${borderRadius}px`,
				'--border-width': `${borderWidth}px`,
				'--border-color': `${borderColor}`,
				'--mask-size': `${size}px`,
				'--spotlight-color': `${spotlightColor}`,
				background
			}}
			className={cn(
				'relative h-full w-full overflow-hidden rounded-[var(--border-radius)] transition-all duration-200 hover:brightness-[1.25]',
				isolated && [borderStyles, 'after:opacity-0 after:hover:opacity-100'],
				isolated &&
					spotlight && [spotlightStyles, 'before:opacity-0 before:hover:opacity-100'],
				!isolated && [borderStyles, 'after:opacity-[var(--opacity)]'],
				!isolated && spotlight && [spotlightStyles, 'before:opacity-[var(--opacity)]']
			)}
		>
			<div
				className={cn(
					'absolute inset-[var(--border-width)] z-[2] rounded-[var(--border-radius)]',
					className
				)}
			>
				{children}
			</div>
		</Component>
	);
};

export { MagicCard, MagicContainer };
