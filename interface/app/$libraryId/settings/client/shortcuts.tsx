import { useState } from 'react';
import { Heading } from '../Layout';

export const Component = () => {
	const [syncWithLibrary, setSyncWithLibrary] = useState(true);
	return (
		<>
			{/* I don't care what you think the "right" way to write "keybinds" is, I simply refuse to refer to it as "keybindings" */}
			<Heading title="Shortcuts" description="Available shortcuts" />
			<div>
				<span>General</span>
				<div className="relative mt-5 overflow-x-auto">
					<table className="w-full text-left text-sm text-gray-500 ">
						<thead className="border-white text-xs uppercase text-white">
							<tr>
								<th scope="col" className="px-6 py-3">
									Key
								</th>
								<th scope="col" className="px-6 py-3">
									Description
								</th>
							</tr>
						</thead>
						<tbody>
							<tr className="border-b border-white">
								<th
									scope="row"
									className="whitespace-nowrap px-6 py-4 font-medium text-gray-400"
								>
									<kbd className="border-white-200 rounded-lg border bg-gray-100 px-2 py-1.5 text-xs font-semibold text-gray-800 ">
										Shift
									</kbd>
									<span className="mx-2 text-white">+</span>
									<kbd className="rounded-lg border border-gray-200 bg-gray-100 px-2 py-1.5 text-xs font-semibold text-gray-800 ">
										Tab
									</kbd>
								</th>
								<td className="px-6 py-4 text-white">
									Navigate to interactive elements
								</td>
							</tr>
							<tr className="text-white">
								<th
									scope="row"
									className="whitespace-nowra inline-flex items-center px-6 py-4 font-medium text-gray-500 "
								>
									<kbd className="mr-1 inline-flex items-center rounded-lg border border-gray-200 bg-gray-100 px-2 py-1.5 text-gray-800 ">
										<svg
											className="h-2.5 w-2.5"
											aria-hidden="true"
											xmlns="http://www.w3.org/2000/svg"
											fill="currentColor"
											viewBox="0 0 16 10"
										>
											<path d="M9.207 1A2 2 0 0 0 6.38 1L.793 6.586A2 2 0 0 0 2.207 10H13.38a2 2 0 0 0 1.414-3.414L9.207 1Z" />
										</svg>
										<span className="sr-only">Arrow key up</span>
									</kbd>
									<kbd className="inline-flex items-center rounded-lg border border-gray-200 bg-gray-100 px-2 py-1.5 text-gray-800 ">
										<svg
											className="h-2.5 w-2.5"
											aria-hidden="true"
											xmlns="http://www.w3.org/2000/svg"
											fill="currentColor"
											viewBox="0 0 16 10"
										>
											<path d="M15.434 1.235A2 2 0 0 0 13.586 0H2.414A2 2 0 0 0 1 3.414L6.586 9a2 2 0 0 0 2.828 0L15 3.414a2 2 0 0 0 .434-2.179Z" />
										</svg>
										<span className="sr-only">Arrow key down</span>
									</kbd>
									<span className="mx-2 text-white">or</span>
									<kbd className="mr-1 inline-flex items-center rounded-lg border border-gray-200 bg-gray-100 px-2 py-1.5 text-gray-800 ">
										<svg
											className="h-2.5 w-2.5"
											aria-hidden="true"
											xmlns="http://www.w3.org/2000/svg"
											fill="currentColor"
											viewBox="0 0 10 16"
										>
											<path d="M8.766.566A2 2 0 0 0 6.586 1L1 6.586a2 2 0 0 0 0 2.828L6.586 15A2 2 0 0 0 10 13.586V2.414A2 2 0 0 0 8.766.566Z" />
										</svg>
										<span className="sr-only">Arrow key left</span>
									</kbd>
									<kbd className="inline-flex items-center rounded-lg border border-gray-200 bg-gray-100 px-2 py-1.5 text-gray-800 ">
										<svg
											className="h-2.5 w-2.5"
											aria-hidden="true"
											xmlns="http://www.w3.org/2000/svg"
											fill="currentColor"
											viewBox="0 0 10 16"
										>
											<path d="M3.414 1A2 2 0 0 0 0 2.414v11.172A2 2 0 0 0 3.414 15L9 9.414a2 2 0 0 0 0-2.828L3.414 1Z" />
										</svg>
										<span className="sr-only">Arrow key right</span>
									</kbd>
								</th>
								<td className="px-6 py-4">
									Choose and activate previous/next tab.
								</td>
							</tr>
						</tbody>
					</table>
				</div>
			</div>
			<div>
				<span>Explorer</span>
				<div className="relative mt-5 overflow-x-auto">
					<table className="w-full text-left text-sm text-gray-500 ">
						<thead className="border-white text-xs uppercase text-white ">
							<tr>
								<th scope="col" className="px-6 py-3">
									Key
								</th>
								<th scope="col" className="px-6 py-3">
									Description
								</th>
							</tr>
						</thead>
						<tbody>
							<tr className="border-b border-white">
								<th
									scope="row"
									className="whitespace-nowrap px-6 py-4 font-medium text-gray-400"
								>
									<kbd className="border-white-200 rounded-lg border bg-gray-100 px-2 py-1.5 text-xs font-semibold text-gray-800 ">
										Shift
									</kbd>
									<span className="mx-2 text-white">+</span>
									<kbd className="rounded-lg border border-gray-200 bg-gray-100 px-2 py-1.5 text-xs font-semibold text-gray-800 ">
										Tab
									</kbd>
								</th>
								<td className="px-6 py-4 text-white">
									Navigate to interactive elements
								</td>
							</tr>
							<tr className="border-b border-white text-white">
								<th
									scope="row"
									className="whitespace-nowra inline-flex items-center px-6 py-4 font-medium text-gray-500"
								>
									<kbd className="mr-1 inline-flex items-center rounded-lg border border-gray-200 bg-gray-100 px-2 py-1.5 text-gray-800 ">
										<svg
											className="h-2.5 w-2.5"
											aria-hidden="true"
											xmlns="http://www.w3.org/2000/svg"
											fill="currentColor"
											viewBox="0 0 16 10"
										>
											<path d="M9.207 1A2 2 0 0 0 6.38 1L.793 6.586A2 2 0 0 0 2.207 10H13.38a2 2 0 0 0 1.414-3.414L9.207 1Z" />
										</svg>
										<span className="sr-only">Arrow key up</span>
									</kbd>
									<kbd className="inline-flex items-center rounded-lg border border-gray-200 bg-gray-100 px-2 py-1.5 text-gray-800 ">
										<svg
											className="h-2.5 w-2.5"
											aria-hidden="true"
											xmlns="http://www.w3.org/2000/svg"
											fill="currentColor"
											viewBox="0 0 16 10"
										>
											<path d="M15.434 1.235A2 2 0 0 0 13.586 0H2.414A2 2 0 0 0 1 3.414L6.586 9a2 2 0 0 0 2.828 0L15 3.414a2 2 0 0 0 .434-2.179Z" />
										</svg>
										<span className="sr-only">Arrow key down</span>
									</kbd>
									<span className="mx-2 text-white">or</span>
									<kbd className="mr-1 inline-flex items-center rounded-lg border border-gray-200 bg-gray-100 px-2 py-1.5 text-gray-800 ">
										<svg
											className="h-2.5 w-2.5"
											aria-hidden="true"
											xmlns="http://www.w3.org/2000/svg"
											fill="currentColor"
											viewBox="0 0 10 16"
										>
											<path d="M8.766.566A2 2 0 0 0 6.586 1L1 6.586a2 2 0 0 0 0 2.828L6.586 15A2 2 0 0 0 10 13.586V2.414A2 2 0 0 0 8.766.566Z" />
										</svg>
										<span className="sr-only">Arrow key left</span>
									</kbd>
									<kbd className="inline-flex items-center rounded-lg border border-gray-200 bg-gray-100 px-2 py-1.5 text-gray-800 ">
										<svg
											className="h-2.5 w-2.5"
											aria-hidden="true"
											xmlns="http://www.w3.org/2000/svg"
											fill="currentColor"
											viewBox="0 0 10 16"
										>
											<path d="M3.414 1A2 2 0 0 0 0 2.414v11.172A2 2 0 0 0 3.414 15L9 9.414a2 2 0 0 0 0-2.828L3.414 1Z" />
										</svg>
										<span className="sr-only">Arrow key right</span>
									</kbd>
								</th>
								<td className="px-6 py-4">
									Choose and activate previous/next tab.
								</td>
							</tr>
							<tr className="border-b border-white text-white">
								<th
									scope="row"
									className="whitespace-nowra inline-flex items-center px-6 py-4 font-medium text-gray-500"
								>
									<kbd className="mr-1 inline-flex items-center rounded-lg border border-gray-200 bg-gray-100 px-2 py-1.5 text-gray-800 ">
										<svg
											className="h-2.5 w-2.5"
											aria-hidden="true"
											xmlns="http://www.w3.org/2000/svg"
											fill="currentColor"
											viewBox="0 0 16 10"
										>
											<path d="M9.207 1A2 2 0 0 0 6.38 1L.793 6.586A2 2 0 0 0 2.207 10H13.38a2 2 0 0 0 1.414-3.414L9.207 1Z" />
										</svg>
										<span className="sr-only">Arrow key up</span>
									</kbd>
									<kbd className="inline-flex items-center rounded-lg border border-gray-200 bg-gray-100 px-2 py-1.5 text-gray-800 ">
										<svg
											className="h-2.5 w-2.5"
											aria-hidden="true"
											xmlns="http://www.w3.org/2000/svg"
											fill="currentColor"
											viewBox="0 0 16 10"
										>
											<path d="M15.434 1.235A2 2 0 0 0 13.586 0H2.414A2 2 0 0 0 1 3.414L6.586 9a2 2 0 0 0 2.828 0L15 3.414a2 2 0 0 0 .434-2.179Z" />
										</svg>
										<span className="sr-only">Arrow key down</span>
									</kbd>
									<span className="mx-2 text-white">or</span>
									<kbd className="mr-1 inline-flex items-center rounded-lg border border-gray-200 bg-gray-100 px-2 py-1.5 text-gray-800 ">
										<svg
											className="h-2.5 w-2.5"
											aria-hidden="true"
											xmlns="http://www.w3.org/2000/svg"
											fill="currentColor"
											viewBox="0 0 10 16"
										>
											<path d="M8.766.566A2 2 0 0 0 6.586 1L1 6.586a2 2 0 0 0 0 2.828L6.586 15A2 2 0 0 0 10 13.586V2.414A2 2 0 0 0 8.766.566Z" />
										</svg>
										<span className="sr-only">Arrow key left</span>
									</kbd>
									<kbd className="inline-flex items-center rounded-lg border border-gray-200 bg-gray-100 px-2 py-1.5 text-gray-800 ">
										<svg
											className="h-2.5 w-2.5"
											aria-hidden="true"
											xmlns="http://www.w3.org/2000/svg"
											fill="currentColor"
											viewBox="0 0 10 16"
										>
											<path d="M3.414 1A2 2 0 0 0 0 2.414v11.172A2 2 0 0 0 3.414 15L9 9.414a2 2 0 0 0 0-2.828L3.414 1Z" />
										</svg>
										<span className="sr-only">Arrow key right</span>
									</kbd>
								</th>
								<td className="px-6 py-4">
									Choose and activate previous/next tab.
								</td>
							</tr>
						</tbody>
					</table>
				</div>
			</div>
		</>
	);
};
