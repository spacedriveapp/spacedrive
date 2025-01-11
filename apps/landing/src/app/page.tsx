import {
	Bento,
	Companies,
	Explorer,
	Github,
	Header,
	Mobile,
	Search,
	Security,
	Tags,
	Vdfs
} from '~/app/page-sections';

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
				<Mobile />
				<Explorer />
				<Tags />
				<Vdfs />
				<Bento />
				<Security />
				<Search />
				<Github />
			</div>
		</>
	);
}
