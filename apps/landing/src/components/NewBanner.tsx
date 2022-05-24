import React from 'react';

export default function NewBanner() {
	return (
		<a href="https://blog.spacedrive.com/spacedrive-funding-announcement" target="_blank">
			<div className="text-xs sm:text-base fade-in-whats-new px-5 py-1.5 bg-opacity-50 cursor-pointer mx-3 sm:mx-0 mb-5 flex flex-row bg-gray-800 hover:bg-gray-750 border border-gray-600 hover:border-gray-550 rounded-full transition">
				<span className="truncate text-gray-350">Spacedrive raises $1.9M lead by OSS Capital</span>
				<div className="w-[1px] mx-4 h-22 bg-gray-500" />
				<span className="flex-shrink-0 font-semibold text-transparent bg-clip-text bg-gradient-to-r from-primary-400 to-blue-600">
					Read post →
				</span>
			</div>
		</a>
	);
}
