import copyOgg from "./copy.ogg";
import copyMp3 from "./copy.mp3";
import startupOgg from "./startup.ogg";
import startupMp3 from "./startup.mp3";

/**
 * Play a sound effect
 * Uses OGG with MP3 fallback for broad compatibility
 */
function playSound(oggSrc: string, mp3Src: string, volume = 0.5) {
	const audio = new Audio();

	// Try OGG first (better quality, smaller size)
	if (audio.canPlayType("audio/ogg; codecs=vorbis")) {
		audio.src = oggSrc;
	} else {
		audio.src = mp3Src;
	}

	audio.volume = volume;
	audio.play().catch((err) => {
		console.warn("Failed to play sound:", err);
	});
}

export const sounds = {
	copy: () => playSound(copyOgg, copyMp3, 0.3),
	startup: () => playSound(startupOgg, startupMp3, 0.5),
};
