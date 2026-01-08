import copyMp3 from "./copy.mp3";
import copyOgg from "./copy.ogg";
import jobDoneMp3 from "./job-done.mp3";
import jobDoneOgg from "./job-done.ogg";
import pairingMp3 from "./pairing.mp3";
import pairingOgg from "./pairing.ogg";
import splatMp3 from "./splat.mp3";
import splatOgg from "./splat.ogg";
import splatTriggerMp3 from "./splat-trigger.mp3";
import splatTriggerOgg from "./splat-trigger.ogg";
import startupMp3 from "./startup.mp3";
import startupOgg from "./startup.ogg";

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
  pairing: () => playSound(pairingOgg, pairingMp3, 0.5),
  splat: () => playSound(splatOgg, splatMp3, 0.05),
  splatTrigger: () => playSound(splatTriggerOgg, splatTriggerMp3, 0.3),
  jobDone: () => playSound(jobDoneOgg, jobDoneMp3, 0.4),
};
