# Sounds

UI sound effects for Spacedrive.

## Adding New Sounds

### 1. Convert to Web Formats

Use ffmpeg to convert your audio file to both OGG and MP3 for broad compatibility:

```bash
# From WAV, AIFF, or any audio format
ffmpeg -i YourSound.wav -acodec libvorbis -q:a 4 output.ogg
ffmpeg -i YourSound.wav -acodec libmp3lame -q:a 4 output.mp3
```

**Quality settings:**
- `-q:a 4` = Good quality (range: 0-10, lower is better)
- `-q:a 2` = Higher quality, larger file
- `-q:a 6` = Lower quality, smaller file

**Tips:**
- Keep sounds short (< 2 seconds for UI feedback)
- Normalize volume to avoid jarring loud sounds
- OGG usually gives better quality at smaller sizes than MP3

### 2. Add to index.ts

```typescript
import newSoundOgg from "./new-sound.ogg";
import newSoundMp3 from "./new-sound.mp3";

export const sounds = {
  copy: () => playSound(copyOgg, copyMp3, 0.3),
  newSound: () => playSound(newSoundOgg, newSoundMp3, 0.5), // Add here
};
```

### 3. Use in Components

```typescript
import { sounds } from "@sd/assets/sounds";

// Play the sound
sounds.copy();
sounds.newSound();
```

## Current Sounds

- **copy** - Plays when file copy/move completes (volume: 30%)
- **startup** - Plays when the app first loads (volume: 50%)

## Advanced Conversion

### Adjust Volume

```bash
# Reduce volume by 50%
ffmpeg -i input.wav -af "volume=0.5" output.ogg

# Normalize audio (make consistent volume)
ffmpeg -i input.wav -af "loudnorm" output.ogg
```

### Trim Length

```bash
# Take first 1 second
ffmpeg -i input.wav -t 1 output.ogg

# Take from 0.5s to 1.5s
ffmpeg -i input.wav -ss 0.5 -t 1 output.ogg
```

### Fade In/Out

```bash
# Fade in over 0.2s, fade out over 0.2s
ffmpeg -i input.wav -af "afade=t=in:st=0:d=0.2,afade=t=out:st=0.8:d=0.2" output.ogg
```

## File Size Guidelines

- UI feedback sounds: < 50KB
- Notification sounds: < 100KB
- Background music: < 500KB

Keep sounds minimal to avoid bloating the app bundle.
