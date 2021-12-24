export interface ImageMeta {
  type: 'image';
  dimensions: {
    width: string;
    height: string;
  };
  color_space: string;
  aperture: number;
  exposure_mode: number;
  exposure_program: number;
  f_number: number;
  flash: boolean;
  focal_length: number;
  has_alpha_channel: boolean;
  iso_speed: number;
  orientation: number;
  metering_mode: number;
}

export interface VideoMeta {
  type: 'video';
  codecs: Array<string>;
  bitrate: {
    video: string;
    audio: string;
  };
  duration_seconds: number;
}

export interface AudioMeta {}
