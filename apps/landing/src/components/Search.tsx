import { useRouter } from 'next/navigation';
import { Search as SMSearch, useSearch, useShortcut } from 'searchmate-react';
import 'searchmate-react/css';
import { SearchInput } from '@sd/ui';

const SM_APP_ID = process.env.NEXT_PUBLIC_SM_APP_ID || '';

export default function Search() {
	const { push } = useRouter();
	const { isOpen, onClose, onOpen } = useSearch();
	useShortcut({
		callback: onOpen,
		isOpen,
		key: 'k',
		withCtrl: true
	});

	const origin = typeof window !== 'undefined' ? window.location.origin : '';
	const prefix = origin ? `${origin}/docs` : '';

	const handleNavigate = (url: string, withCtrl: boolean) => {
		if (withCtrl) {
			window.open(url, '_blank');
		} else {
			push(url);
		}
	};

	return (
		<>
			<div onClick={onOpen}>
				<SearchInput
					placeholder="Search..."
					disabled
					right={<span className="pr-2 text-xs font-semibold text-gray-400">⌘K</span>}
				/>
			</div>
			<SMSearch
				appId={SM_APP_ID}
				isOpen={isOpen}
				onClose={onClose}
				urlPrefix={prefix}
				overrideNavigateToResult={handleNavigate}
			/>
		</>
	);
}
