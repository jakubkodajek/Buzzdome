# Voice samples

What each voice of the app's voice model (Supertonic 3 by Supertone, via
sherpa-onnx; `crates/voice/src/model.rs`) sounds like, in Czech and in
English, so the quizmaster can choose a voice before downloading the model
(spec SN-10). `make_samples.py` renders them with the model itself; the
sentences are the project's own. Speech the model makes is ours to use
under its OpenRAIL-M licence.

Run `python3 app/src/audio/voices/make_samples.py <unpacked model folder>`
to make them again, for example after the model changes.
