import { MediaLocation, MediaMetadata, MediaTime } from '@sd/client';
import Accordion from '~/components/Accordion';
import { MetaData } from './index';

interface Props {
	data: MediaMetadata;
}

function formatMediaTime(loc: MediaTime): string | null {
	if (loc === 'Undefined') return null;
	if ('Utc' in loc) return loc.Utc;
	if ('Naive' in loc) return loc.Naive;
	return null;
}

function formatLocation(loc: MediaLocation): string {
	// Stackoverflow says the `+` strips the trailing zeros or something so it's important, I think
	return `${+loc.latitude.toFixed(2)}, ${+loc.longitude.toFixed(2)}`;
}

function MediaData({ data }: Props) {
	return data.type === 'Image' ? (
		<div className="flex flex-col gap-0 py-2">
			<Accordion
				containerVariant='apple'
				boxVariant='apple'
				titleVariant='apple'
				title="More info"
			>
				<MetaData label="Date" value={formatMediaTime(data.date_taken)} />
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
