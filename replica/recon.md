# Recon map: QuizXpress 8.5 (Windows desktop + mobile)

Scope: the core quiz-night loop. Author a quiz, run it live on a projector,
players answer on their phones, the quizmaster controls the show from a
second device, scores and leaderboards. Game formats beyond regular
questions, mini games and hardware are mapped but scoped as later work.
For: Jakub's own non-profit events. Not for sale.
Target platform of the clone: web (browser on any OS), no install for anyone.
Date: 2026-10-05

Every fact below comes from the sources table. Section numbers like `§5.7.2`
point into the v8.5 manual (`replica/sources/user-manual-v8.5.pdf`; PDF page
numbers equal the printed page numbers). Page renders of the key screens are
in `replica/screens/`, named by screen ID. Descriptions are paraphrased. Nothing here (text,
screenshots, sounds, skins, names) may be copied into the app.

## Sources

| # | source | URL | notes |
| --- | --- | --- | --- |
| 1 | User manual 8.5 (254 pp., July 2025) | provided by the user, local copy in `replica/sources/user-manual-v8.5.pdf`; Markdown per chapter in `replica/sources/manual-v8.5/` | primary source, chapters 3 to 5 read in full |
| 2 | User manual 6.5 (169 pp., Sept 2023) | https://www.quizxpress.com/wp-content/uploads/2020/06/User-Manual-QuizXpress-6.pdf | older; local copy in `replica/sources/`, not read in detail |
| 3 | Quick start guide | https://www.quizxpress.com/howto/quick-start-guide/ | first-run flow, keyboard keys, Director app pairing |
| 4 | "How it works" page | https://www.quizxpress.com/game-show-software-how-it-works/ | module overview, "up to 5000 players" claim |
| 5 | YouTube channel Game Show Crew | https://www.youtube.com/@GameShowCrew | not reviewed yet; best source for real click-by-click flows |

Not used: the trial installer (Windows only, and its EULA was not reviewed).

## Core loop

The quizmaster runs a live quiz show: questions appear on a big screen, every
player answers on a device, the system judges and scores, and the quizmaster
paces the show and builds suspense around the scores.

## Product parts in the original

| part | what it is | role in our web version |
| --- | --- | --- |
| Studio | Windows editor with a ribbon UI for building quiz files (`.qx`) | **Studio** page in the browser |
| Setup | separate settings app: hardware, mobile, sounds, screens, advanced flags, end-of-question actions | merged into per-event **Show settings** in Studio |
| Live (Quiz Show) | full-screen DirectX player shown on the projector | **Host screen** (browser tab on the projector) |
| Director (desktop) | control panel on the laptop screen while Live runs on the projector | **Director** page (laptop, tablet or phone) |
| Director (mobile app) | iOS/Android remote with 5 tabs | same **Director** page, responsive |
| Smart Buzzer app / buzzerpad web | player keypad on the phone | **Player** page (no install) |
| Analyzer, Solo, Virtual Keypad, Buzzer Test | results viewer, trade-show kiosk, test tools | results export; simulated players for testing |

## Screens

States are what the manual describes or shows. "?" means not documented.

### Studio (authoring)

| ID | screen | route / how to reach | purpose | key components | states seen |
| --- | --- | --- | --- | --- | --- |
| S01 | Studio start | launch Studio | open example or recent quiz | welcome screen with "Browse Examples" (quick start guide, not in the manual), backstage Recent with examples (§3.2.2) | filled |
| S02 | Studio editor | main window | build and edit slides | ribbon, thumbnails panel, slide canvas, properties grid, notes box, status bar | empty quiz, filled, multi-select, contextual tab shown |
| S03 | Backstage menu | FILE tab or F1 | file-level actions | New (choose aspect ratio), Recent, Import, Export, Print, Info (license, release notes), Pack/Unpack, Options | filled |
| S04 | Edit question form | double-click slide or right-click > Edit | edit question and all answers in one form | question field, answer fields | filled |
| S05 | Table mode | VIEW > Table mode | edit all slides as rows | grid: type, question, answers, correct answer, notes, optional property columns; Excel import/export of Q, A1..A6, CORRECT, NOTES | filled |
| S06 | Category view | VIEW > Category | define reusable slide looks | category slides, properties grid | empty, filled |
| S07 | Quiz Wizard | HOME > Magic > Quiz Wizard | generate a slide or a whole round skeleton | multi-step wizard: round type, topics/points, time, points, style | step pages |
| S08 | Type-specific dialogs | set a slide type or right-click > Configure | configure special slides | End-of-round action, Wager %, Survey editor, Speed bonus, Set correctness, Set order, mini game config, bingo pattern editor | filled |
| S09 | Quiz options | FILE > Options | quiz-wide settings | target ratio 4:3 / 16:9 / 16:10, quality, expiry/DRM, author history, jokers | filled |
| S10 | Slide preview | F5 or "Preview Slide" | full-screen preview of one slide, trivia board, ladder or bingo card | rendered slide | filled |

### Setup (show settings)

| ID | screen | route / how to reach | purpose | key components | states seen |
| --- | --- | --- | --- | --- | --- |
| S11 | Quiz Setup | Start menu or Studio HOME > External | configure the show | left: list of settings files (active one bold); top icon tabs Buzzers, Mobile, Teams (only with physical keypads, §4.3.2), Sound, Screens, Advanced, End of Question, Analyzer; bottom: Reset, Import, Export, Save, Save As, Close | no buzzer chosen (help hint), hardware test OK (green check) |
| S12 | Teams and groups | Setup > Teams; Groups button | name teams, define groups and group scoring | team table (keypad, name, avatar), Generate Names, Import, Save to PDF, groups list with keypad ranges | filled, validation error on bad range |

### Live (host screen on the projector)

| ID | screen | route / how to reach | purpose | key components | states seen |
| --- | --- | --- | --- | --- | --- |
| S13 | Launch checker | starting a quiz | warn about missing media or folders | warning list, Ignore, Quit | no issues (skipped), warnings |
| S14 | Welcome / lobby | first screen after launch | gather the audience | header text + logo, intro video/picture, photo slideshow, footer strip, next button bottom right; PIN indicator; Q shows QR overlay | filled, receiver missing (blinking keypad icon) |
| S15 | Registration | after welcome (buzzers only) | register participating keypads | grid of keypad numbers ticked on press (<100), compact list (>100), name carousel (Sony), start button | empty, partly, all registered |
| S16 | 3-2-1 countdown | after registration | build suspense | countdown video | playing, skipped with Space |
| S17 | Question: everyone answers | a question slide with voting on | ask a question everyone answers | top information panel (clock, responders grid, answered count, points), question text, picture, answers A-F | answers hidden/revealed with delay, counting down, paused, all answered, time out, judged |
| S18 | Question: fastest finger | question slide with voting off | first to buzz answers | info panel with buzz-in timestamps, bottom buzz bar (name, answer, points), big check or cross overlay | waiting for buzz, buzzed, judged right, judged wrong then resumed |
| S19 | Judged question | end of a question | show the outcome | correct answer highlighted with a check, info panel turns into correct/incorrect/no-answer bars | correct shown, correct hidden (setting) |
| S20 | Leaderboard | key S (Shift+S reversed) | intermediate ranking | 2 columns x 10 rows: rank, name, score; scroll with arrows | top 20, scrolled, reversed |
| S21 | Score control screens | keys E, R, T, W | view and edit scores in place | team list with last response, time, points; toolbar: disable, resume, -1, +1, reset all, auto-scroll, confirm, cancel | browsing, edited, read-only (W) |
| S22 | Charts and grids | C (answers), G (groups), A (right/wrong grid) | show response distribution | bar chart per answer, group chart, per-player grid | shown, hidden |
| S23 | Round scores / round winners | Ctrl+S, Ctrl+E | per-round results | ranking list, winner per round | shown |
| S24 | End of round | end-of-round slide | close a round | designed slide + action: show scores, show round scores, reset scores, dismiss lowest, quizmaster picks losers (blinking red selection, undo and play buttons), auto-dismiss | billboard only, with dismissal |
| S25 | Special slide screens | slide types | other formats | billboard, test question, demographic join, last man standing, audience response chart, wager options, majority rules, trivia board grid, trivia ladder, survey board, speed round, bingo balls | per type |
| S26 | Final scores | after last slide | announce winners | drum roll, top 3 revealed one by one, then full ranking; bottom 5 nav buttons + close (hidden when Director is open); group/individual toggle | auto reveal, step-by-step reveal (setting) |
| S27 | Quizmaster hints | F1 when idle, or automatic | teach the quizmaster the available commands | button list with shortcut keys | shown, switched off (Setup flag, on by default; the manual advises turning it off for real shows, §4.6) |

### Director (quizmaster control)

| ID | screen | route / how to reach | purpose | key components | states seen |
| --- | --- | --- | --- | --- | --- |
| S28 | Director: Questions | key D, or opens on start (setting) | navigate the quiz | left command panel; tabs; toolbar (Go to question, Go to end, Edit question, Edit timing, Layout horizontal/vertical); slide table; preview pane with answers, correct order and notes | current, visited, counting down (navigation disabled) |
| S29 | Director: Teams | Teams tab | manage players and scores | table (name, group, keypad, score, last response, total time, enabled); toolbar and right-click menu (see Layout) | filled, multi-select edit, bingo card dialog |
| S30 | Director: Responses | Responses tab | see answers arriving live | keypad, name, response, time | live, mini game choices |
| S31 | Director: Analyzer / Round scores | tabs | per-question history; scores per round | history grid; collapsible round nodes | only with rounds |
| S32 | Director: Extensions | Extensions tab | launch mini games on the fly | plugin list, configurations, Configure / Launch / End | idle, running (extra buttons in command panel) |
| S33 | Director: Media Player | Media Player tab | play media outside slides | media list; full-screen video or picture; mp3; promo image to phones; replay slide media; volume | filled |
| S34 | Director: Mobile | Mobile tab | manage phone connections | device list; Select Jingle, Send Message, Blacklist, Latency Test, Pair Host Remote (QR), Settings, Chat; current PIN | connected, slow (red latency), chat open |
| S35 | Mobile Director app | phone app, paired by QR from Setup | control the show while walking around | 5 bottom tabs: Remote, Mini games, Questions, Players, Sound pad | not connected, connected |

### Player (phone)

| ID | screen | route / how to reach | purpose | key components | states seen |
| --- | --- | --- | --- | --- | --- |
| S36 | Join | buzzerpad URL + PIN, or QR (PIN prefilled), or app "Join Wi-Fi game" | enter the game | PIN field, name field, GO; app: Wi-Fi join and PIN join buttons; optional ticket code; group picker | empty, PIN prefilled, Wi-Fi game found |
| S37 | Jingle picker | after join, if jingles are configured | pick a personal sound | list of sounds, Confirm | filled |
| S38 | Waiting / promo | between questions | keep players oriented | branded welcome image, promo image sent from Director, messages | waiting, promo shown |
| S39 | Answer screen | question running | answer | header (keypad number, name), status bar (n/N, trophy points, clock blinking last 5 s), question, input by type (see Layout) | answering, answered ("We received answer D"), locked out, time up |
| S40 | Feedback | question judged | show the result | big check / cross, correct answer for letter questions, bonus-points animation, points won | correct, incorrect, bonus |
| S41 | Scores on phone | leaderboard / final | personal standing | intermediate scores; final: announcement, top 3 if in it, own score and rank | intermediate, final |
| S42 | Extras | when enabled | engagement | joker menu, thumbs up/down poll (10 s), chat button, bingo card | per feature |

## Layout and controls of the key screens

The button layouts below are read from the manual's screenshots and text.
They are reference for what controls exist and where users expect them, not a
design to copy. Our layouts are designed fresh.

### S02 Studio editor (§3.2, PDF p. 14)

```
+--------------------------------------------------------------------------+
| [quick access: save, undo, redo]                                         |
| FILE | HOME | INSERT | QUESTION | DESIGN | VIEW | QUIZ SHOW | (contextual)|
| [ribbon groups for the active tab .....................................]  |
+--------+--------------------------------------------------+--------------+
| thumb- |                                                  | properties   |
| nails  |   slide canvas (WYSIWYG, 16:9 by default)        | grid         |
| (drag  |   question box on top                            | Appearance   |
|  to    |   picture left, answers A-D stacked on the right |  Behavior    |
|  re-   |   as coloured bars with a letter badge           |  General     |
|  order)|   correct answer marked with a check             |  Points      |
|        +--------------------------------------------------+              |
|        | quizmaster notes (free text)                     |              |
+--------+--------------------------------------------------+--------------+
| status bar (update link)                                                 |
+--------------------------------------------------------------------------+
```

- Ribbon tabs: FILE (backstage), HOME (clipboard, font, alignment and size,
  order, banners, shuffle, find, Quiz Wizard, External tools, Options,
  Translate), INSERT (layout gallery of quiz slides with the special rounds on
  the bottom row: mini game, trivia board, trivia ladder, survey, bingo; plus
  picture, sound, video, text, shape, speech, chart), QUESTION (question
  type with End-of-round and Wager buttons next to it, points with Speed
  Bonus, input mode, judge automatically/manually, topic, configure), DESIGN
  (change layout, categories gallery, styles), VIEW (Properties, Notes,
  Graphical/Table mode, Category view), QUIZ SHOW (Test, Test current
  question, Run Production, Quickstart, Simulate keypads).
- Contextual tabs appear only while an element is selected: PICTURE, SHAPE,
  SPEECH, CHART, AUDIO, VIDEO, TRIVIA BOARD, TRIVIA LADDER, BINGO, WORD GAME,
  TRIVIA FEUD (§3.2.1).
- Thumbnail context menu: cut, copy, paste, delete, select all, select
  questions, select by slide type. Slides copy between Studio windows (§3.2.4).
- Marking the correct answer: select it and tick "Correct answer" in the
  grid, right-click > Set correct answer, or the last column in table mode
  (§3.4.1.1).
- Shortcuts: F1 backstage (§3.2.2), F5 preview (§3.2.6.1), Ctrl+A select all
  slides (§3.2.4), Ctrl+F find, Shift+F3 toggle text case, Ctrl+Z / Ctrl+Y
  undo/redo (§3.14).

### S17 / S18 Live question screen (§5.4.5, PDF pp. 199, 201)

```
+---------+------------------------------------+-----------+------------+
| (clock) | responders: grid of keypad numbers | people 34 | trophy 10  |
|  18     | -> numbers -> names of the last few|  answered |  points    |
|  1/7    |                                    |           |            |
+---------+------------------------------------+-----------+------------+
|                 Question text, centred, large                         |
|  +-----------+   (A) answer ..........................                |
|  |  picture  |   (B) answer ..........................                |
|  |           |   (C) answer ..........................                |
|  +-----------+   (D) answer ..........................                |
+-----------------------------------------------------------------------+
| fastest finger only: [ team name ............ (A) +06 ]  + big ✓ / ✗  |
+-----------------------------------------------------------------------+
```

- The responders indicator narrows as answers arrive: a grid of squares,
  then the remaining keypad numbers, then the remaining player names, so the
  quizmaster can call out who is missing (§5.4.5).
- After judging, the panel shows three bars: correct (green), incorrect
  (red), no answer (§5.4.5).
- Fastest finger replaces the answered count with buzz-in boxes ordered by
  time, fastest left (§5.4.5.1).

### S20 Leaderboard (PDF p. 205)

Two columns of ten rows (rank, team name, score), ranks 1 to 3 in gold,
silver and bronze. Scroll with the arrow keys or remote. Shift+S starts from
the bottom; Ctrl+S shows the current round (§5.4.6).

### S28 / S29 Director (§5.7, PDF pp. 214 to 218)

```
+----------------+--------------------------------------------------------+
| [Next (Space)] | Questions | Teams | Responses | Round scores |          |
| [Fullscreen M] | Extensions | Media Player | Mobile                     |
| [context cmds] +--------------------------------------------------------+
|  e.g. Right /  | [Go to question][Go to end][Edit question][Edit timing] |
|  Wrong, Start  | [Layout: horizontal | vertical]                         |
|  faceoff ...   +-------------------------------+------------------------+
|                | # | thumbnail | type | topic | question preview:      |
|                | rows: visited / current       | text, answers A-F,     |
|                |                               | order numbers, notes   |
| [Exit] (red)   |                               |                        |
+----------------+-------------------------------+------------------------+
```

- Left command panel: large stacked buttons that change with the show state
  (next, fullscreen, judge, mini game steps, survey round controls); Exit at
  the bottom. Right-click it for dark mode (§5.7, §5.7.10).
- Questions table colours: desktop Director marks visited slides green and
  the current one yellow (§5.7.1); the mobile app uses the opposite (§5.2.4.4).
- Teams toolbar: enable/disable, disable others, edit, add player mid-show,
  delete, show bingo card, undo last question's score changes, reset scores,
  edit scores in a table, import results from a previous quiz; multi-select
  rows to add points or set a group in one go (§5.7.2).
- Teams right-click: add/subtract points with remembered presets, "add and
  animate" (big check on screen), "subtract and animate" (big cross), enter
  a vote, AI roast (§5.7.2).
- Ranking ties are broken by total answering time (§5.7.2). Buzzdome
  counts only the time of right answers (`docs/design.md`, Ranking).

### S35 Mobile Director app (§5.2.4.4, PDF pp. 189 to 190)

- Remote tab: a 3-column grid of large tiles. Commands documented:
  leaderboard (with E/R/T variants), response grid, chart, round scores,
  round winners, pause/resume, stop, scroll up/down, next, and Right/Wrong
  for verbal answers (thumbs down red, thumbs up green on the bottom row),
  volume. Exact tile positions are not readable in the manual's screenshot.
- Questions tab: slide list with visited/current colours, a counter strip
  after each everyone-answers question (correct / incorrect / no response),
  bottom toolbar: pause, stop, chart, leaderboard, current/list toggle, next.
- Players tab: edit, add, disable, delete. Sound pad tab: up to 12 sound
  buttons from a folder set in Setup.

### S39 Player answer screen (§5.2.4.1, PDF pp. 184 to 185)

```
+--------------------------------------+
| 1                       John Doe     |   keypad number, player name
| 16/20      (trophy) 24      (clock)  |   status bar: index, points, time
| What is the first letter of ...?     |
| [ input for this question type ]     |
+--------------------------------------+
```

Input by type: one big round buzzer (fastest finger); full-width coloured
answer buttons stacked vertically (multiple choice, also with pictures or a
picture effect); a 5x5 letter grid A to Z with X/Y sharing a key, optionally
shuffled or moving (first letter); text field + Submit (full text, can have
several fields); numeric keypad 0-9, +/-, decimal, backspace + Submit
(number); drag to order then send (ordered). Feedback: big check or cross,
bonus-points animation (§5.2.4.1).

## Flows

```
F01 Quizmaster builds a multiple-choice question
    S02 INSERT > pick layout -> type question -> type answers
        -> right-click correct answer > Set correct -> QUESTION tab: time, points -> save
    happy path clicks: ~10
    edge: more than 4 answers, multiple correct answers, ordered answers, picture answers

F02 Quizmaster imports questions from a spreadsheet
    S03 Import > Excel -> S02 slides created per row, layout chosen from filled columns
    happy path clicks: ~4
    edge: worksheet = round (billboard inserted), {prefix} sets slide type, missing media path

F03 Quizmaster starts a show with phone players
    S02 QUIZ SHOW > Run Production -> S13 launch checker -> S14 welcome (Q shows QR + PIN)
        -> players join (F04) -> Space -> S16 3-2-1 -> S17 first question
    happy path clicks: 2 clicks + 2 keys
    edge: launch warnings, players arriving late, internet down (local Wi-Fi mode)

F04 Player joins and answers
    scan QR -> S36 name (PIN prefilled) -> GO -> S38 wait -> S39 answer -> S40 feedback -> S41 scores
    happy path actions: 3 to join, 1 to 2 per question
    edge: duplicate name, phone sleeps and reconnects, joins mid-show, waiting room, ticket code,
          answer revised while clock runs, answer arrives after time up

F05 Everyone-answers question runs
    S17 answers revealed (optional delay, one by one) -> clock runs -> all answered or time up
        -> S19 judged -> end-of-question actions on Space: show correct, response bar,
           scores, chart -> next slide
    edge: clock paused (P), untimed question ended with Space, nobody answered

F06 Fastest-finger question with a verbal answer
    S18 players buzz -> first buzzer shown with jingle -> quizmaster judges Y/N
        (keyboard, Director Right/Wrong, or poll the room with thumbs up/down)
        -> right: points, question ends / wrong: penalty, buzzer locked out, clock resumes
           (with steal points on: no penalty for the buzzer, others gain it, question ends)
    edge: steal points, next-in-line within 2 s, group lockout, everyone wrong

F07 Quizmaster runs the show from the Director
    D -> S28 next / go to question / edit question live -> S29 fix a score, disable a player,
        undo last question -> S30 see who is missing
    edge: navigation blocked while the clock runs, mini game running

F08 Round ends
    end-of-round slide -> S24 action: show scores / round scores / dismiss lowest
        (tie: quizmaster picks) / manual dismissal / reset scores -> next round
    edge: last-man-standing players re-join after the round

F09 Final scores
    after last slide -> S26 drum roll -> (Space) 3rd -> 2nd -> 1st -> full ranking
    edge: group scores vs individual scores, more than one page of players

F10 Recover after a crash
    restart Live -> "resume?" Yes -> continues from the snapshot saved after the last
        completed question (scores, assignments, current question) (§9.1)
    edge: test mode has no recovery (§3.15, §9.1); a question in progress is replayed
```

## Components

| component | variants | states | used on |
| --- | --- | --- | --- |
| Answer option bar | letter A-F, text, picture, coloured per letter | hidden, revealed, revealed one by one, chosen count, correct (check), incorrect | S02, S17, S19, S39 |
| Countdown clock | round badge with seconds and n/N | running, paused, untimed, last 5 s blinking (phone), ended | S17, S18, S39 |
| Information panel | everyone-answers, fastest finger, judged | counting, buzzed, result bars | S17, S18, S19 |
| Responders indicator | grid, number list, name list | narrowing as answers arrive | S17 |
| Points badge | trophy + number | static, decreasing over time | S17, S39 |
| Buzz bar + verdict overlay | name, answer, points, check or cross | pending, right, wrong | S18 |
| Leaderboard row | rank, name, score, medal colours | normal, highlighted, selected for dismissal (blinking red) | S20, S23, S24, S26 |
| Response chart | per answer, per group, right/wrong grid | live, final, correct highlighted or not | S19, S22 |
| PIN / QR overlay | corner indicator, full QR | shown, hidden | S14, S17 |
| Slide canvas | edit, preview, live | selected element, multi-select, snapping | S02, S10 |
| Thumbnail list | slide thumbnails with number | current, multi-select, dragging | S02 |
| Property grid | grouped key/value editor | single, multi-select | S02 |
| Ribbon / toolbar | tabs, groups, contextual tabs | enabled, disabled | S02, S28 |
| Command panel | large stacked buttons | context-dependent | S28 |
| Data table | slides, teams, responses, devices | sorted, multi-select, editable cells | S05, S28 to S34 |
| Player input | buzzer, choice buttons, letter grid, text, numeric keypad, ordering list | idle, submitted, locked, disabled | S39 |
| Feedback card | correct, incorrect, bonus | animated | S40 |
| Toast / device message | "answer received", "no answer received", points | transient | S39 |
| Modal dialog | edit question, edit player, settings, confirm | open, validation error | many |

## Inferred data model

```
Quiz        id, title, aspect_ratio (4:3 | 16:9 | 16:10), author, last_modified_by,
            languages (primary, secondary?), joker_config, expiry?
            evidence: §3.2.2, §3.9, §3.14.4, §3.14.11
            confidence: high

Category    id, quiz_id, name, background, fonts, colours, style
            evidence: §3.3
            confidence: high

Slide       id, quiz_id, position, slide_key (routing id), category_id, layout,
            type (question | billboard | end_of_round | test | demographic |
                  last_man_standing | audience_response | wager | majority_rules |
                  minigame | trivia_board | trivia_ladder | survey | speed_round | bingo),
            voting (true = everyone answers, false = fastest finger),
            input_mode (choice | default (verbal) | number | letter | text),
            answer_time_s (0 = untimed), timer_start (auto | manual),
            delay_before_answers_s, answers_reveal (at once | one by one), delay_between_answers_s,
            auto_judge, lockout_on_wrong, steal_points, continue_after_correct,
            correct_mode (single | multi_one_reply | multi_many_reply | ordered),
            response_mode, ordered_mode (exact | partial),
            correct_value (number, range "80-90", or texts separated by ";"),
            number_eval (exact | nearest_wins, nearest_winners n),
            text_eval (exact | fuzzy), tolerance, text_response_limit,
            points_start, points_end, penalty_start, penalty_end, no_answer_penalty,
            bonus_points [1st, 2nd, 3rd], response_limit, allow_revise (override),
            show_correct (global | never | always), shuffle_mode, next_slide, tie_mode,
            topic, notes, background, transition, end_of_round_action, demographic_group,
            jokers_enabled
            evidence: §3.2.5.1 property table, §3.4, §3.14.2
            confidence: high

Answer      id, slide_id, position (A..F, A7/A8 for surveys), text, picture?,
            is_correct, correctness_pct, order_index, wager_pct, next_slide
            evidence: §3.2.5.3, §3.4.1.2, §5.4.5.8, §3.14.2, §3.10.1 (A7/A8), §3.2.5.15
            confidence: high

SlideItem   id, slide_id, kind (question_text | picture | shape | text | sound | video |
            speech | chart | word_game), geometry, style, timing
            (reveal animation, delay, visibility on reveal), media ref (embedded | linked)
            evidence: §3.2.5.2 to §3.2.5.11
            confidence: high

ShowSettings  profile name, server mode (cloud | local), mobile options, sounds per event,
            welcome texts, logo, slideshow, skin, flags (full-screen slides, 3-2-1 intro,
            individual feedback < 20 players, top-3 reveal, show correct answer, randomise %,
            hints, auto advance, auto-disable idle players, next in line, auto restart,
            full countdown, allow revise, hide points), end_of_question_actions [ordered]
            evidence: §4.2 to §4.7
            confidence: high

Session     id, quiz_id (or playlist), pin (5 digits), status, current_slide, started_at,
            snapshot for recovery, server_delay_s
            evidence: §5.2.3, §9.1 (snapshot after every completed question), §5.7.8.3
            confidence: medium (structure guessed, behaviour documented)

Player      id, session_id, keypad_no, name, group_id?, enabled, blacklisted, score,
            total_answer_time, jingle, jokers_left, device_id
            evidence: §5.7.2, §5.7.8, §3.14.4
            confidence: high

Group       id, session_id, name, ff_mode (lockout | no_lockout),
            vote_mode (first_only | sum | average)
            evidence: §4.3.2.1, §4.2.2.1
            confidence: high

Response    id, session_id, slide_id, player_id, payload (choice ids | ordered ids |
            number | letter | texts[]), received_at, is_correct, points_delta, bonus_rank
            evidence: §5.7.3, §5.7.4, Analyzer export §7
            confidence: medium

ScoreEvent  id, session_id, player_id, slide_id?, delta, reason (answer | bonus | manual |
            undo | wager | steal)
            evidence: undo last question, manual edits §5.7.2
            confidence: guess (needed to support undo)

Playlist    id, name, quiz_ids [ordered]; scores carry over
            evidence: §3.14.14
            confidence: high
```

Relationships: Quiz 1-n Slide, Slide 1-n Answer, Slide 1-n SlideItem,
Quiz 1-n Category, Session n-1 Quiz, Session 1-n Player, Session 1-n Group,
Player 1-n Response, Slide 1-n Response, Player 1-n ScoreEvent.
Rounds are derived: the slides between end-of-round slides (§5.4.5.3).

## Scoring rules worth getting right

- Points for a correct answer interpolate linearly from `points_start` to
  `points_end` over the countdown; penalties do the same (§3.2.5.1, §3.4.1).
- Speed bonus goes to the 1st, 2nd and 3rd fastest correct players (§3.4.1).
- Multiple correct answers carry a correctness % each; ordered answers can
  score partially by the share of answers in the right position (§3.4.1.2).
- Number answers: exact, a range, or nearest-wins for the top n (§3.4.1.3.1).
- Text answers: case-insensitive, alternatives split by `;`, optional fuzzy
  match with a tolerance (§3.4.1.3.2). For Czech we need diacritics-aware
  matching (our addition).
- Steal points: a wrong buzz costs the buzzer nothing; the slide's penalty
  points are added to every other player and the question ends (§3.2.5.1).
  The same table has a second, contradicting row (others take the points from
  the buzzer). Pick one rule on purpose.
- Wager: players bet a % (or fixed points) of their score on the next
  question (§5.4.5.8).
- Groups: fastest finger with or without group lockout; voting counts first
  answer only, sums or averages (§4.3.2.1).
- Ties in the ranking break on total answering time (§5.7.2); Buzzdome
  counts only right answers' time (`docs/design.md`, Ranking).

## Feature matrix

See `features.csv`. Must: 30, should: 32, could: 43, skip: 10.
Plus 3 rows the original does not have (`original = no`).

## Out of scope (cannot or should not be cloned)

- Proprietary hardware: QuizXpress keypads, buzzers, poles, Reply and Sony
  receivers, Galaxy LED devices, wireless scoreboards. Generic USB buzzers
  through WebHID stay a possible later addition.
- Their hosted services: buzzerpad.com/.eu servers, Quiz Center
  subscriptions, branded custom domains, AI credits shop.
- Their content and assets: example quizzes, sounds, music, skins, mini game
  3D scenes, avatars, slide styles.
- Licensing, activation, DRM and quiz expiry.
- TV format names (Jeopardy!, Who Wants to Be a Millionaire?, Family Feud,
  Wheel of Fortune) and the QuizXpress name. Rounds get generic names.
- QuizXpress Solo (trade-show lead capture) and the PowerPoint toggle.

## Size

Screens 42 (original), flows 10, entities 11. Hard parts:

1. Realtime sync to many phones with fair fastest-finger ordering and
   reconnects (phones sleep, Wi-Fi drops).
2. Crash-proof session state: resume a live show where it stopped.
3. The WYSIWYG slide editor, if we build one. A form-based editor with
   fixed layouts is far cheaper.
4. Scoring rules (interpolation, bonuses, partial credit, fuzzy text, groups).
5. Media playback on the host screen (audio and video in sync with the clock).

Size: the full original is XL. The must-have slice (form-based editor,
everyone-answers and fastest-finger questions, phones, Director, scores) is
M. (This sizing predates the 2026-10-05 move to a desktop app with
hardware buzzers; the plan now is `ROADMAP.md`.)
