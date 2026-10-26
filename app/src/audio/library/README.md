# The app's own sounds

The show's sound effects and music (spec SN-1, SN-2). `make_sounds.py`
makes every file here from sine waves, filters and seeded noise: no
recording, sample or other third-party material goes into them, so they
are the project's own work, like its code. `sounds.json` lists each
sound's kind and length, and for music where its loop starts again.

To change a sound, change its function in `make_sounds.py` and run it
(`python3 app/src/audio/library/make_sounds.py`, with NumPy and an
`ffmpeg` that has libmp3lame); it rewrites every file. A new sound also
needs its moment in `SLOT_SOUNDS` and its name in `SOUND_NAME`
(`../library.ts`) and in both languages of `i18n.ts`;
`library.test.ts` checks that.
