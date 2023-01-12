import { ScreenHeading } from '@sd/ui';

export default function MediaScreen() {
	return (
		<div className="flex flex-col w-full h-screen p-5 custom-scroll page-scroll app-background">
			<div className="flex flex-col pb-4 space-y-5">
				<p className="px-5 py-3 text-sm border rounded-md shadow-sm border-app-line bg-app-box ">
					<b>Note: </b>This is a pre-alpha build of Spacedrive, many features are yet to be
					functional.
				</p>
			</div>
			<ScreenHeading>Media</ScreenHeading>
		</div>
	);
}
