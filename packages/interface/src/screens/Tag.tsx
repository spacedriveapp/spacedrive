import React, { useEffect } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';

export const TagScreen: React.FC<{}> = () => {
	let [searchParams] = useSearchParams();
	let path = searchParams.get('path') || '';

	let { id } = useParams();

	return (
		<div className="w-full p-5">
			<p className="px-5 py-3 mb-3 text-sm text-gray-400 rounded-md bg-gray-50 dark:text-gray-400 dark:bg-gray-600">
				<b>Note: </b>This is a pre-alpha build of Spacedrive, many features are yet to be
				functional.
			</p>
		</div>
	);
};
