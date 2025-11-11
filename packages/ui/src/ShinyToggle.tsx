import { motion } from 'framer-motion';
import { ReactNode } from 'react';

interface ShinyToggleOption<T extends string> {
	value: T;
	label: ReactNode;
	count?: number;
}

interface ShinyToggleProps<T extends string> {
	value: T;
	onChange: (value: T) => void;
	options: ShinyToggleOption<T>[];
	className?: string;
}

export function ShinyToggle<T extends string>({
	value,
	onChange,
	options,
	className = ''
}: ShinyToggleProps<T>) {
	return (
		<div className={`inline-flex gap-1 rounded-full bg-app-box p-1 border border-app-line ${className}`}>
			{options.map((option) => (
				<button
					key={option.value}
					type="button"
					onClick={() => onChange(option.value)}
					className="relative px-3 py-1.5 text-sm font-medium transition-colors"
				>
					{value === option.value && (
						<motion.div
							layoutId="shinyToggleActive"
							className="absolute inset-0 rounded-full bg-gradient-to-b from-[#42B2FD]/80 to-[#0078F0]/80 shadow-[0_0_0.75rem_hsl(207_100%_65%/30%)] border-2 border-[#42B2FD]/40"
							transition={{ type: 'spring', bounce: 0.2, duration: 0.6 }}
						/>
					)}
					<span className={`relative ${value === option.value ? 'text-white' : 'text-ink-dull'}`}>
						{option.label}
						{option.count !== undefined && ` (${option.count})`}
					</span>
				</button>
			))}
		</div>
	);
}
