import { MediaLocation, MediaMetadata } from '@sd/client';
import Accordion from '~/components/Accordion';
import { MetaData } from './index';

interface Props {
	data: MediaMetadata;
}

function formatLocation(loc: MediaLocation): string {
	return `${loc.latitude}, ${loc.longitude}`;
}

function MediaData({ data }: Props) {
	return data.type === 'Image' ? (
		<div className="flex flex-col gap-0 py-2">
			<Accordion
				containerClassName="flex flex-col gap-1 px-4 rounded-b-none"
				titleClassName="px-4 pt-0 pb-1 !justify-end gap-2 flex-row-reverse text-ink-dull"
				className="rounded-none border-0 bg-transparent py-0"
				title="More info"
			>
				<MetaData
					label="Date"
					value={'value' in data.date_taken ? data.date_taken.value : null}
				/>
				<MetaData label="Type" value={data.type} />
				<MetaData
					label="Location"
					value={data.location ? formatLocation(data.location) : null}
				/>
				<MetaData
					label="Dimensions"
					value={
						<>
							{data.dimensions.width} x {data.dimensions.height}
						</>
					}
				/>
				<MetaData label="Device" value={data.camera_data.device_make} />
				<MetaData label="Model" value={data.camera_data.device_model} />
				<MetaData label="Orientation" value={data.camera_data.orientation} />
				<MetaData label="Color profile" value={data.camera_data.color_profile} />
				<MetaData label="Color space" value={data.camera_data.color_space} />
				<MetaData label="Flash" value={data.camera_data.flash?.mode} />
				<MetaData label="Zoom" value={data.camera_data.zoom} />
				<MetaData label="Iso" value={data.camera_data.iso} />
				<MetaData label="Software" value={data.camera_data.software} />
			</Accordion>
		</div>
	) : null;
}

export default MediaData;
