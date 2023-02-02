import { Listbox as ListboxPrimitive } from '@headlessui/react';
import clsx from 'clsx';
import { Check, Sun } from 'phosphor-react';
import { useEffect, useState } from 'react';

interface ListboxOption {
	option: string;
	description?: string;
	key: string;
}

export default function Listbox(props: { options: ListboxOption[]; className?: string }) {
	const [selected, setSelected] = useState(props.options[0]);

	useEffect(() => {
		if (!selected) {
			setSelected(props.options[0]);
		}
	}, [props.options, selected]);

	return (
		<>
			<ListboxPrimitive value={selected} onChange={setSelected}>
				<div className="relative w-full">
					<ListboxPrimitive.Button
						className={clsx(
							`relative w-full cursor-default rounded-lg bg-white py-2 pl-3 pr-10 
								text-left shadow-md focus:outline-none focus-visible:border-indigo-500 focus-visible:ring-2 
								focus-visible:ring-white focus-visible:ring-opacity-75 focus-visible:ring-offset-2 
								focus-visible:ring-offset-orange-300 dark:bg-gray-500 sm:text-sm`,
							props.className
						)}
					>
						{selected?.option ? (
							<span className="block truncate">{selected?.option}</span>
						) : (
							<span className="block truncate opacity-70">Nothing selected...</span>
						)}

						<span className="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-2">
							<Sun className="h-5 w-5 text-gray-400" aria-hidden="true" />
						</span>
					</ListboxPrimitive.Button>

					<ListboxPrimitive.Options
						className={`
							absolute mt-1 max-h-60 w-full overflow-auto rounded-md
							bg-white text-base shadow-lg ring-1 ring-black 
							ring-opacity-5 focus:outline-none dark:bg-gray-500 sm:text-sm
						`}
					>
						{props.options.map((option, index) => (
							<ListboxPrimitive.Option
								key={option.key}
								className={({ active }) =>
									`relative m-1 cursor-default select-none rounded py-2 pl-8 pr-4 focus:outline-none dark:text-white  ${
										active
											? 'text-accent bg-accent'
											: 'text-gray-900 dark:hover:bg-gray-600 dark:hover:bg-opacity-20'
									}`
								}
								value={option}
							>
								{({ selected }) => (
									<>
										<span className={`block truncate ${selected ? 'font-medium' : 'font-normal'}`}>
											{option.option}
											{option.description && (
												<span
													className={clsx(
														'ml-3 text-xs leading-5 text-gray-300',
														selected && 'text-white'
													)}
												>
													{option.description}
												</span>
											)}
										</span>

										{selected ? (
											<span className="absolute inset-y-0 left-0 flex items-center pl-2 text-white">
												<Check className="h-5 w-5" aria-hidden="true" />
											</span>
										) : null}
									</>
								)}
							</ListboxPrimitive.Option>
						))}
					</ListboxPrimitive.Options>
				</div>
			</ListboxPrimitive>
		</>
	);
}
