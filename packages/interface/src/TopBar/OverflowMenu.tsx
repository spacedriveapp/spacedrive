import { useState, useRef, useEffect } from "react";
import { createPortal } from "react-dom";
import { DotsThree } from "@phosphor-icons/react";
import { motion } from "framer-motion";
import { TopBarButton } from "@sd/ui";
import { TopBarItem } from "./Context";

interface OverflowButtonProps {
	items: TopBarItem[];
}

export function OverflowButton({ items }: OverflowButtonProps) {
	const [isOpen, setIsOpen] = useState(false);
	const buttonRef = useRef<HTMLButtonElement>(null);
	const panelRef = useRef<HTMLDivElement>(null);
	const [position, setPosition] = useState({ top: 0, left: 0 });

	useEffect(() => {
		if (isOpen && buttonRef.current) {
			const rect = buttonRef.current.getBoundingClientRect();
			setPosition({
				top: rect.bottom + 8,
				left: rect.left,
			});
		}
	}, [isOpen]);

	useEffect(() => {
		const handleClickOutside = (e: MouseEvent) => {
			if (
				panelRef.current &&
				buttonRef.current &&
				!panelRef.current.contains(e.target as Node) &&
				!buttonRef.current.contains(e.target as Node)
			) {
				setIsOpen(false);
			}
		};

		if (isOpen) {
			document.addEventListener("mousedown", handleClickOutside);
			return () => document.removeEventListener("mousedown", handleClickOutside);
		}
	}, [isOpen]);

	if (items.length === 0) return null;

	return (
		<>
			<TopBarButton
				ref={buttonRef}
				icon={DotsThree}
				onClick={() => setIsOpen(!isOpen)}
				active={isOpen}
			/>

			{isOpen &&
				createPortal(
					<motion.div
						ref={panelRef}
						initial={{ opacity: 0, y: -10 }}
						animate={{ opacity: 1, y: 0 }}
						exit={{ opacity: 0, y: -10 }}
						transition={{ duration: 0.15 }}
						style={{
							position: "fixed",
							top: `${position.top}px`,
							left: `${position.left}px`,
						}}
						className="min-w-[180px] rounded-lg bg-app border border-app-line shadow-2xl py-1 z-50"
					>
						{items.map((item) => (
							<button
								key={item.id}
								onClick={() => {
									item.onClick?.();
									setIsOpen(false);
								}}
								className="w-full px-3 py-2 text-left text-sm text-menu-ink hover:bg-app-hover/50 transition-colors"
							>
								{item.label}
							</button>
						))}
					</motion.div>,
					document.body
				)}
		</>
	);
}