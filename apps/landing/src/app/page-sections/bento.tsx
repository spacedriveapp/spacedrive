import { Icon } from '../Icon';

const PRETTY_BENTO_CLASS = `bento-border-left relative flex flex-col rounded-[10px] bg-[radial-gradient(66.79%_83.82%_at_0%_3.69%,#1B1D25_0%,#15161C_100%)] items-center justify-center w-[150px] h-[150px]`;
const BENTO_TITLE_CLASS = 'text-md font-medium mt-2 text-ink-dull';

export function Bento() {
	return (
		<div className="container mx-auto flex items-center gap-2 p-4">
			<div className={PRETTY_BENTO_CLASS}>
				<Icon name="Spacedrop1" size={80} />
				<h3 className={BENTO_TITLE_CLASS}>Spacedrop</h3>
			</div>
			<div className={PRETTY_BENTO_CLASS}>
				<Icon name="Sync" size={80} />
				<h3 className={BENTO_TITLE_CLASS}>P2P Sync</h3>
			</div>
			<div className={PRETTY_BENTO_CLASS}>
				<Icon name="Package" size={80} />
				<h3 className={BENTO_TITLE_CLASS}>Archival Tools</h3>
			</div>
			<div className={PRETTY_BENTO_CLASS}>
				<Icon name="Lock" size={80} />
				<h3 className={BENTO_TITLE_CLASS}>File Encryption</h3>
			</div>
			<div className={PRETTY_BENTO_CLASS}>
				<Icon name="CollectionSparkle" size={80} />
				<h3 className={BENTO_TITLE_CLASS}>AI Labeling</h3>
			</div>
			<div className={PRETTY_BENTO_CLASS}>
				<Icon name="Video" size={80} />
				<h3 className={BENTO_TITLE_CLASS}>Video Encoding</h3>
			</div>
			<div className={PRETTY_BENTO_CLASS}>
				<Icon name="Movie" size={80} />
				<h3 className={BENTO_TITLE_CLASS}>In-app Player</h3>
			</div>
			<div className={PRETTY_BENTO_CLASS}>
				<Icon name="Database" size={80} />
				<h3 className={BENTO_TITLE_CLASS}>Local Database</h3>
			</div>
		</div>
	);
}
