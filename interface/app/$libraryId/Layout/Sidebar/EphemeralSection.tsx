import { Link } from 'react-router-dom';
import { Folder, SubtleButton } from '~/components';
import SidebarLink from './Link';
import Section from './Section';

export const EphemeralSection = () => {
	// TODO: Query backend for home
	const home = '/Users/vitor';

	// TODO: Query backend for disks

	return (
		<>
			<Section
				name="Explore"
				actionArea={
					<Link to="settings/ephemeral">
						<SubtleButton />
					</Link>
				}
			>
				<SidebarLink
					to={`ephemeral?path=${home}`}
					className="group relative w-full border border-transparent"
				>
					<div className="shrink-0 grow-0">
						<Folder size={18} />
					</div>

					<span className="truncate">Home</span>
				</SidebarLink>
			</Section>
		</>
	);
};
