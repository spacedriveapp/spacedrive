import {Plus} from '@phosphor-icons/react';
import {CircleButton} from '@spacedrive/primitives';
import {useNavigate} from 'react-router-dom';
import {TopBarItem, TopBarPortal} from '../../TopBar';
import {useAddStorageDialog} from '../explorer/components/AddStorageModal';

interface OverviewTopBarProps {
	libraryName?: string;
}

/** Home owns the first storage action; library scope lives in the sidebar. */
export function OverviewTopBar(_props: OverviewTopBarProps) {
	const navigate = useNavigate();

	const addStorage = () => {
		useAddStorageDialog((path) => {
			navigate(
				`/explorer?path=${encodeURIComponent(JSON.stringify(path))}`
			);
		});
	};

	return (
		<TopBarPortal
			left={
				<TopBarItem id="home-title" label="Home" priority="high">
					<h1 className="text-ink text-xl font-bold">Home</h1>
				</TopBarItem>
			}
			right={
				<TopBarItem
					id="add-storage"
					label="Add Storage"
					priority="high"
				>
					<CircleButton
						icon={Plus}
						className="!bg-accent hover:!bg-accent-deep !text-white"
						onClick={addStorage}
					>
						Add Storage
					</CircleButton>
				</TopBarItem>
			}
		/>
	);
}
