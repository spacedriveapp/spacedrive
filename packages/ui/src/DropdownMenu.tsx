'use client';

import clsx from 'clsx';
import { type ReactNode, type PropsWithChildren, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';

// Minimal base styles - customize via className prop
const baseMenuStyles = 'overflow-hidden w-full';
const baseItemWrapperStyles = 'w-full';
const baseItemStyles = 'flex w-full items-center cursor-pointer';
const baseSeparatorStyles = 'border-b';

// ===== ROOT COMPONENT =====
interface DropdownMenuProps {
	trigger: ReactNode;
	className?: string;
	onOpenChange?: (open: boolean) => void;
}

function Root({
	onOpenChange,
	trigger,
	className,
	children,
}: PropsWithChildren<DropdownMenuProps>) {
	const [isOpen, setIsOpen] = useState(false);

	const handleOpenChange = (open: boolean) => {
		setIsOpen(open);
		onOpenChange?.(open);
	};

	return (
		<div className="w-full">
			<div onClick={() => handleOpenChange(!isOpen)}>{trigger}</div>

			<AnimatePresence>
				{isOpen && (
					<motion.div
						initial={{ height: 0, opacity: 0 }}
						animate={{ height: 'auto', opacity: 1 }}
						exit={{ height: 0, opacity: 0 }}
						transition={{ duration: 0.15, ease: [0.25, 1, 0.5, 1] }}
						className="overflow-hidden"
					>
						<div className={clsx(baseMenuStyles, 'mt-1', className)}>
							{children}
						</div>
					</motion.div>
				)}
			</AnimatePresence>
		</div>
	);
}

// ===== ITEM COMPONENT =====
interface ItemProps {
	icon?: React.ElementType;
	label?: string;
	children?: ReactNode;
	selected?: boolean;
	onClick?: () => void;
	className?: string;
}

function Item({ icon: Icon, label, children, selected, className, onClick }: ItemProps) {
	return (
		<div className={baseItemWrapperStyles}>
			<button
				onClick={onClick}
				className={clsx(baseItemStyles, className)}
			>
				{Icon && <Icon className="mr-2 size-4" weight="bold" />}
				{label || children}
			</button>
		</div>
	);
}

// ===== SEPARATOR COMPONENT =====
function Separator({ className }: { className?: string }) {
	return <div className={clsx(baseSeparatorStyles, className)} />;
}

export const DropdownMenu = {
	Root,
	Item,
	Separator,
};
