import { CheckIcon } from '@heroicons/react/solid';
import React from 'react';

export const PaymentTier = () => {
	return (
		<div className="relative w-full h-full flex flex-col sm:flex-row items-center  justify-center mt-4">
			<div className=" flex flex-col items-center w-full max-w-md p-4 rounded-xl mx-2 my-2 sm:my-2 bg-gray-650  hover:bg-gray-750  border border-gray-600 hover:border-gray-550  duration-300 ease-in-out">
				<h1 className="text-2xl font-bold sm:text-3xl pt-4 pb-2">Open Source</h1>
				<p className="text-gray-450 text-md">For all those tinkerers.</p>
				<p className="text-6xl font-black pt-4">
					$0 <small className="text-base font-light text-gray-450">/month</small>
				</p>
				<ul className="mb-8 space-y-4 max-w-sm text-left items-center mt-12 ">
					<li className="flex items-center space-x-3">
						<CheckIcon className="flex-shrink-0 w-5 h-5 text-green-400 dark:text-green-400" />
						<span className="text-white">
							Sync your photos, videos and files across multiple devices.
						</span>
					</li>
					<li className="flex items-center space-x-3">
						<CheckIcon className="flex-shrink-0 w-5 h-5 text-green-500 dark:text-green-500" />
						<span className="text-white">Customize with custom themes.</span>
					</li>
					<li className="flex items-center space-x-3">
						<CheckIcon className="flex-shrink-0 w-5 h-5 text-green-500 dark:text-green-400" />
						<span>Total privacy and control.</span>
					</li>
					<li className="flex items-center space-x-3">
						<CheckIcon className="flex-shrink-0 w-5 h-5 text-green-500 dark:text-green-400" />
						<span className="text-white">
							Sync across storage providers such as AWS, GDrive etc.
						</span>
					</li>
					<li className="flex items-center space-x-3">
						<CheckIcon className="flex-shrink-0 w-5 h-5 text-gray-450 dark:text-gray-450" />
						<span className="text-gray-450">
							We provide you with a SpaceDrive to store your data on.
						</span>
					</li>
					<li className="flex items-center space-x-3">
						<CheckIcon className="flex-shrink-0 w-5 h-5 text-gray-450 dark:text-gray-450" />
						<span className="text-gray-450">Other features coming soon...</span>
					</li>
				</ul>
			</div>

			<div className=" flex flex-col items-center w-full max-w-md p-4 rounded-xl mx-2 my-2 sm:my-2 bg-gray-650  hover:bg-gray-750  border border-gray-600 hover:border-gray-550  duration-300 ease-in-out">
				<h1 className="text-2xl font-bold sm:text-3xl pt-4 pb-2">Personal</h1>
				<p className="text-gray-450 text-md">(Coming Soon)</p>
				<p className="text-6xl font-black pt-4">
					$?? <small className="text-base font-light text-gray-450">/month</small>
				</p>
				<div></div>
				<ul className="mb-8 space-y-4 max-w-sm text-left items-center mt-12">
					<li className="flex items-center space-x-3">
						<CheckIcon className="flex-shrink-0 w-5 h-5 text-green-400 dark:text-green-400" />
						<span className="text-white">
							Sync your photos, videos and files across multiple devices.
						</span>
					</li>
					<li className="flex items-center space-x-3">
						<CheckIcon className="flex-shrink-0 w-5 h-5 text-green-500 dark:text-green-500" />
						<span className="text-white">Customize with custom themes.</span>
					</li>
					<li className="flex items-center space-x-3">
						<CheckIcon className="flex-shrink-0 w-5 h-5 text-green-500 dark:text-green-400" />
						<span>Total privacy and control.</span>
					</li>
					<li className="flex items-center space-x-3">
						<CheckIcon className="flex-shrink-0 w-5 h-5 text-green-500 dark:text-green-400" />
						<span className="text-white">
							Sync across storage providers such as AWS, GDrive etc.
						</span>
					</li>
					<li className="flex items-center space-x-3">
						<CheckIcon className="flex-shrink-0 w-5 h-5 text-green-500 dark:text-green-400" />
						<span className="text-white">
							We provide you with a SpaceDrive to store your data on.
						</span>
					</li>
					<li className="flex items-center space-x-3">
						<CheckIcon className="flex-shrink-0 w-5 h-5 text-green-500 dark:text-green-400" />
						<span className="text-white">Other features coming soon...</span>
					</li>
				</ul>
			</div>
		</div>
	);
};
