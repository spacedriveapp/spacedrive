import { getIcon, getIconByName, getLayeredIcon, IconTypes } from '@sd/assets/util';
import { Assistant, Explorer, Features, Github, Header, Search, Vdfs } from '~/app/page-sections';

import { Icon } from './Icon';
import { Bento } from './page-sections/bento';
import { Companies } from './page-sections/companies';
import Mobile from './page-sections/mobile';
import Tags from './page-sections/tags';

export const metadata = {
	title: 'Spacedrive — Sync, manage, and discover. Across all your devices.',
	description:
		'Combine your drives and clouds into one database that you can organize and explore from any device. Designed for creators, hoarders and the painfully disorganized.',
	keywords:
		'files,file manager,spacedrive,file explorer,vdfs,distributed filesystem,cas,content addressable storage,virtual filesystem,photos app, video organizer,video encoder,tags,tag based filesystem',
	authors: {
		name: 'Spacedrive Technology Inc.',
		url: 'https://spacedrive.com'
	}
};

export default function Page() {
	return (
		<>
			<Header />
			<div className="flex flex-col gap-20 md:gap-[150px]">
				<Companies />
				<Explorer />
				<Tags />
				<Bento />
				<Vdfs />
				<Search />
				{/* <Assistant /> */}
				<Github />
			</div>
		</>
	);
}
