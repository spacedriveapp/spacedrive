import { useState, useRef, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { MagnifyingGlass } from "@phosphor-icons/react";
import { TopBarButton, SearchBar } from "@sd/ui";

interface ExpandableSearchButtonProps {
	value: string;
	onChange: (value: string) => void;
	onClear: () => void;
	placeholder?: string;
}

export function ExpandableSearchButton({
	value,
	onChange,
	onClear,
	placeholder = "Search...",
}: ExpandableSearchButtonProps) {
	const [isExpanded, setIsExpanded] = useState(false);
	const containerRef = useRef<HTMLDivElement>(null);
	const inputRef = useRef<HTMLInputElement>(null);

	// Expand if there's a value
	useEffect(() => {
		if (value) {
			setIsExpanded(true);
		}
	}, [value]);

	// Collapse when clicking outside
	useEffect(() => {
		const handleClickOutside = (event: MouseEvent) => {
			if (
				containerRef.current &&
				!containerRef.current.contains(event.target as Node) &&
				isExpanded &&
				!value
			) {
				setIsExpanded(false);
			}
		};

		if (isExpanded) {
			document.addEventListener("mousedown", handleClickOutside);
			return () => {
				document.removeEventListener("mousedown", handleClickOutside);
			};
		}
	}, [isExpanded, value]);

	// Handle button click
	const handleButtonClick = () => {
		setIsExpanded(true);
	};

	// Focus input after animation completes
	const handleAnimationComplete = () => {
		if (isExpanded && inputRef.current) {
			inputRef.current.focus();
		}
	};

	// Handle input blur - collapse if empty
	const handleBlur = () => {
		if (!value) {
			setIsExpanded(false);
		}
	};

	return (
		<div ref={containerRef}>
			<motion.div
				animate={{
					width: isExpanded ? 256 : 32, // w-64 = 256px, button = 32px
				}}
				transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
				className="overflow-hidden"
				onAnimationComplete={handleAnimationComplete}
			>
				<AnimatePresence mode="wait" initial={false}>
					{!isExpanded ? (
						<motion.div
							key="button"
							initial={{ opacity: 0 }}
							animate={{ opacity: 1 }}
							exit={{ opacity: 0 }}
							transition={{ duration: 0.15 }}
						>
							<TopBarButton icon={MagnifyingGlass} onClick={handleButtonClick} />
						</motion.div>
					) : (
						<motion.div
							key="searchbar"
							initial={{ opacity: 0 }}
							animate={{ opacity: 1 }}
							exit={{ opacity: 0 }}
							transition={{ duration: 0.15 }}
						>
							<SearchBar
								ref={inputRef}
								value={value}
								onChange={onChange}
								onClear={onClear}
								placeholder={placeholder}
								className="w-64"
								onBlur={handleBlur}
								autoFocus
							/>
						</motion.div>
					)}
				</AnimatePresence>
			</motion.div>
		</div>
	);
}