# Voice samples

What each voice of the app's voice model (Supertonic 3 by Supertone, via
sherpa-onnx; `crates/voice/src/model.rs`) sounds like, in Czech and in
English, so the quizmaster can choose a voice before downloading the model
(spec SN-10). `make_samples.py` renders them with the model itself; the
sentences are the project's own. The model is under the BigScience Open
RAIL-M licence (https://huggingface.co/Supertone/supertonic-3/blob/main/LICENSE):
its licensor claims no rights in the speech it makes (§6), and its use
restrictions (Attachment A) apply, among them that machine-generated
speech is disclosed as such and never used to pass for a real person.

Run `python3 app/src/audio/voices/make_samples.py <unpacked model folder>`
to make them again, for example after the model changes.
