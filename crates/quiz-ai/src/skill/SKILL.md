---
name: buzzdome-quiz
description: Writes quizzes for the Buzzdome quiz show app as .quiz files, with questions, answers, points, pictures, sounds and videos. Use when asked to create or change a Buzzdome quiz.
---

# Writing a Buzzdome quiz

Buzzdome is a desktop quiz show: the quizmaster runs it on a laptop, a
projector shows the slides, and players answer with wireless buzzers that
have four answer buttons (A-D) and one big button. You write the quiz; the
quizmaster opens it in Buzzdome's editor, checks it and plays it.

A quiz is a folder with `quiz.json` and its media in `media/`, packed into
one `.quiz` file. The format is in `reference/quiz-format.md` and its JSON
Schema in `reference/quiz.schema.json`. Read the format guide before you
write the first quiz in a conversation.

## Steps

1. Agree on what the quizmaster wants, unless they said it already: the
   topic, the language of the questions, how many rounds and questions,
   whether everyone answers each question (A-D) or the fastest buzzer
   answers aloud, the time and points per question, and which pictures,
   sounds or videos to use. Suggest sensible defaults rather than asking
   many questions.
2. Write `quiz.json` in a new folder, for example `quiz/`, following the
   format guide. Write the questions in the quizmaster's language; any
   language and its accents are fine (the file is UTF-8).
3. Put each picture, sound and video in `quiz/media/` and name it in the
   slide as `media/<file name>`. Use only files you really have: ones the
   quizmaster uploaded, or pictures you made with an image tool. Never
   name a file you do not have. When a slide needs a picture or video you
   cannot provide, leave the field out and say what to add in the slide's
   `notes`, starting with "TODO:", so the quizmaster adds it in the editor.
4. Run `python3 scripts/make_quiz.py quiz -o "<quiz title>.quiz"` (the path
   is relative to this skill's folder). It checks the quiz against the
   schema and its media, and writes the file. Fix what it reports and run
   it again until it writes the file.
5. Give the quizmaster the `.quiz` file, and tell them to open it in
   Buzzdome with File > Open. The editor lists anything left to fix under
   File > Info, and the quiz can be changed there before the show.

If you cannot run code here, write `quiz.json` in full in your answer and
tell the quizmaster to save it as `quiz.json` in a folder, with the media
in a `media` folder next to it, and open that `quiz.json` in Buzzdome.

## A good quiz

- Group questions into rounds: a `billboard` that opens the round, its
  questions, an `explanation` after a question where the answer deserves
  one, and an `endOfRound` slide that shows the scores. End the quiz with a
  `final` slide for the winners.
- Every slide has an `id` unique in the quiz, such as `r1-q3`.
- An `everyone` question has 2 to 4 answers, one per button A-D, and at
  least one right answer in `correct` (indexes from 0). Keep the right
  answer's position varied across questions.
- A `fastestFinger` question has up to 6 answers, or none when players
  answer aloud; the quizmaster judges the answer.
- Use `share` below 100 only on `everyone` questions, for a nearly right
  answer that earns part of the points.
- Keep question texts short enough to read from across a room, and answers
  to a few words.
- Pictures: PNG, JPEG, WebP, GIF or SVG. Sounds: MP3, WAV, OGG or M4A.
  Videos: MP4 (H.264) or WebM. A slide shows a picture or a video, not both.
- Leave out what you do not need: every field has a default, and the
  quiz's theme decides the look unless the quizmaster asks for a style.
