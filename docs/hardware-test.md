# Hardware test: buzzer receiver

A checklist for the first test with the real receiver and buzzers. It checks
specification section 4 and what the Enjoy 303S driver could not check
without the hardware (specification section 11), then smoke-tests the
installed build. Do it on the event laptop (Windows) first, then on a Mac.
Takes about an hour and a half with four buzzers, most of it on section 6.

## What to have

- The receiver, the USB-HID cable, at least four buzzers, charged. Each
  needs its own ID: step 3 of section 3 shows the numbers. The app cannot
  set a 303S buzzer's ID yet (it goes through the receiver, with the
  vendor's software); note any two that share one.
- The app installed from the latest CI run (GitHub > Actions > Build > the run
  > Artifacts: `buzzdome-windows`, `buzzdome-macos` or `buzzdome-linux`),
  or built on the computer with `npm run tauri build` in `app/`.
- On Windows, a build from before the rename to Buzzdome uninstalled first:
  Settings > Apps > **Quiz Show** > Uninstall, with "Delete the application
  data" left unticked, as Buzzdome keeps its quizzes, teams and settings in
  that same folder. The installer tells programs apart by name, so Buzzdome
  installs beside Quiz Show rather than over it, and both would write the
  same show journal. Buzzdome itself runs once (starting it again brings the
  running Director forward), but an old Quiz Show does not step aside.
- The vendor's quiz software closed: only one program can open the receiver.
- A phone that films in slow motion helps with step 6 of section 6.

## 1. Identify

1. Jakub's set is the Enjoy 303S (Galaxy RGB). For any other set, read the
   label under a buzzer base: 215 (dome) or 303S (RGB). The app drives only
   the 303S; with a 215 set, the keyboard and on-screen buzzers work, and
   sections 4 and 9 still apply.
2. Plug the cable into the computer and the receiver. The receiver's power
   light should be on.

## 2. Find the receiver

1. Open the app, Director > **Receiver** tab, click **Refresh**.
2. Unplug the cable, Refresh, plug it back, Refresh. The row that comes and
   goes is the receiver. Note its VID:PID, usage and interface.
3. If several rows share that VID:PID, start with the one whose usage page is
   not `0001` (keyboard or mouse).
4. Click **Use**. Expected: status "Connected ..." (green), with the
   protocol:
   - **Enjoy 303S** for VID:PID `10c4:1819` once the receiver answered its
     set-up: go on with section 3 (learn mode is not offered, section 5
     does not apply). Note how long it took to show.
   - **Enjoy 303S: the receiver does not answer yet** (amber, not green):
     the app opened the cable at 750000 baud and asked the receiver to
     connect, but no answer came (or none to the open that follows); it
     asks again every half second, from the cable's settings. Check its
     power adapter and the RJ45 plug; if it answers then, the status
     changes by itself. If not, unplug and plug the cable again. If it
     stays, note whether anything shows under "Raw data", export the log
     (section 4, step 5) and send it: that is what the set-up gets fixed
     from. Only the keyboard and on-screen buzzers work until then.
   - **raw reports only** for any other receiver: skip section 3.
   - Linux: if it says the device cannot be opened, copy the udev rule from
     the tab, install it as shown, unplug and replug.
   - macOS: if asked about Input Monitoring, allow the app, then restart it.
   - Windows: if it cannot open, make sure no other quiz program is running.

## 3. Enjoy 303S

Only when the status names the Enjoy 303S. The app sets the receiver up by
itself, as described in `docs/design.md` (2.4); these steps check what
could not be checked without the hardware. Keep the Receiver tab open:
"Presses" shows what the app read, with each buzzer's timer ("timer 1234"),
"Raw data" the bytes behind it. Each step says what to note when it does
not go as expected; whatever happens, go on and export the log at the end
(section 4, step 5).

1. **Connect on both computers.** The status above, on the Windows laptop
   and on the Mac. The cable runs at 750000 baud, which nobody has tried
   through macOS yet: note it if the Mac does not connect when Windows does.
   Note how long "Enjoy 303S" takes to show after **Use**.
2. **Plug order.** Unplug the receiver's power adapter, leave the USB cable
   in, and click **Use** (or start the app). Expected: "the receiver does
   not answer yet", in amber. Plug the adapter in. Expected: "Enjoy 303S"
   within a few seconds, by itself. Note anything under "Raw data" at the
   moment the receiver powers up.
3. **Buttons.** With no show running, the buzzers take presses. Press A, B,
   C and D on buzzer 1, then on buzzers 2 to 4. Expected: "buzzer N, button
   X" under "Presses" for each, with the buzzer's own number. Note wrong
   buttons or numbers, and the "Raw data" line of one of them.
4. **The big button.** Press the big button on buzzer 1. Expected: "button
   Big". The vendor's guide does not say which code it sends; the app takes
   5 and 6 for it, the guide's codes for keys a buzzer does not have, and
   no other code, so nothing unknown ever buzzes. If no press shows, copy
   its "Raw data" line: the byte after `32` and the buzzer's number is the
   code (its first hex digit). Fastest finger needs this fixed before a
   show.
5. **Other buttons.** Switch a buzzer off and on with its power button, if
   it has one, and press any other key it has. Expected: no press under
   "Presses". Note any that shows, with its "Raw data" line: it would buzz
   in a show whenever someone switches a buzzer on. Then hold A down for
   three seconds and let go. Expected: one press, none while held and none
   at the release; note how many "Raw data" lines came.
6. **High numbers.** Press A on the highest-numbered buzzer of the set.
   Expected: its own number (up to 64).
7. **Colours.** Section 6 checks each light in a show; there, check also
   that dim white (locked out) is easy to tell from white (first) and from
   red (wrong) next to it, on the stage and in its light, and that it is
   bright enough to see (steps 6 and 12 show them together). Before that,
   or if a light does not change, run
   `hid-monitor` with `lights` (end of this file): every buzzer takes each
   colour in turn, and a second into each, buzzers 2 and 6 alone take the
   next. Note it if those two do not change: the app relies on lighting
   one buzzer right after lighting all of them.

## 4. Record raw reports

1. Press **A** on buzzer 1. Expected: a line appears under "Raw data".
   - If nothing appears: with the Enjoy 303S, the receiver has not answered
     its set-up (section 2, step 4). Otherwise try the other rows with the
     same VID:PID. If none shows reports, note it and go to step 5 (export).
2. On buzzer 1, press A, B, C, D and the big button, each twice, slowly.
   Repeat for buzzer 2 and buzzer 3.
3. Press the big button on buzzers 1 and 2 at the same moment, five times.
4. Hold the big button on buzzer 1 for three seconds.
5. Click **Export log** and save the file.

## 5. Teach buttons (learn mode)

Not with the Enjoy 303S, whose protocol the app knows: the section is
hidden then. Learning could not work on its cable anyway, as the cable
passes nothing until the app has set the receiver up.

1. Under "Teach buttons": buzzer 1, button A, **Start**. Press A three times,
   then **Save**. The form moves on to B, then C, D, the big button, then
   buzzer 2.
2. Teach buzzers 1 to 4.
3. Expected: pressing any taught button now shows "buzzer N, button X" under
   "Presses". If saving shows a conflict, note which buttons; export the log
   again.

## 6. Play

1. Start the demo quiz. In the lobby, press the big button on each buzzer:
   each registers as a team.
2. Everyone-answers question: each buzzer answers with A-D; the counts match.
3. Fastest-finger question: two players press together several times. The
   team shown first must match who the players think was first; note any
   case where it does not.
4. Hold a button: it must count once.
5. Keyboard keys `1`-`0` still work next to the buzzers.

With the Enjoy 303S also. The colours: ready blue, locked out dim white,
first white, right green, wrong red, no question dark. Spare buzzers
switched on light with the others and can join mid-question by pressing, as
a late team: keep them switched off. Times below are read from the exported
log (section 4, step 5; its first column is in milliseconds) or from
`hid-monitor`, as the Receiver tab shows none.

6. **Fastest finger.** When a fastest-finger question starts, every buzzer
   turns blue at the same moment. When one presses, it turns white and the
   others dim white. Note what each light did, and whether the lights change
   later than the press by a delay you can see (film it in slow motion if
   unsure): the app takes all 64 buzzer IDs, which the vendor's guide says
   slows the lights. The winner may show dim white for a moment before
   white; note it if that is long enough to see. Let buzzers 2, 3 and 4,
   and one numbered 5 or higher, each win once: exactly the winner turns
   white each time, none of the others.
7. **The colour a press shows.** For one buzz of step 6, find its "Raw
   data" line: after `27 A5`, the length and `32`, come the buzzer's radio
   ID, the button code, then the colour the buzzer showed. Expected: `43`
   (blue). Brightness aside, a buzz showing any of the app's other colours
   is not counted, and shows as "not counted, the buzzer was not ready":
   if every buzz does that, note the byte, as no buzzer could win.
8. **The buzzer's timer.** In a fastest-finger question, count about five
   seconds after the buzzers turn blue, then press. Note the timer the
   Receiver tab shows for that press: about 5000 means it counts
   milliseconds, which the app assumes. Do it twice, once after about two
   seconds. Then buzz, judge it wrong, and press again a second after the
   others turn blue: about 1000 means the timer starts again when the
   colour changes.
9. **Long questions.** In a fastest-finger question with no time limit,
   press 20 seconds after the buzzers turn blue, and in another 3 minutes
   after. Expected: timers near 20000 and 180000. A far lower one means the
   timer wrapped round, so the unit is not milliseconds: note both.
10. **Two at once.** Two players press together, ten times. The team shown
    first must have the lower timer under "Presses", and match who the
    players think was first; note any case where it does not, with both
    timers. Also note how far apart the two presses arrive, from the
    exported log: the app waits 100 ms for presses made at the same moment,
    so a gap near or above 0.1 s means it must wait longer, and a gap well
    under it that it can wait less.
11. **Too early.** Press before the clock starts or before the buzzers turn
    blue, and hold the big button down while they turn. Expected: neither
    buzzes (the press shows under "Presses" or as not counted, but the
    question goes on), and pressing again once blue does.
12. While a buzz is judged, the other players keep hammering their
    buttons; then judge it wrong. Expected: that buzzer turns red, without
    turning blue in between, the others turn blue again together; no press
    made while they were dim white counts, not even one made just before
    Wrong was clicked, and the first press once they are blue does. The one
    judged wrong cannot buzz again.
13. Judge a buzz right. Expected: that buzzer turns green.
14. **Which buttons buzz.** A fastest-finger question with answers A to D
    (the demo quiz has one): A to D buzz in, and the letter pressed shows
    in the Director's status and on the projector's bar; the big button
    does nothing. A fastest-finger question with no answers: the big button
    buzzes in, A to D do nothing.
15. Everyone-answers question: the buzzers turn blue, and each turns dim
    white once it answered. After the results, the buzzers that answered
    right turn green and the others red. On a question with two answers,
    C, D and the big button do not count as answers.
16. Switch one player off in the Director. Expected: their buzzer turns dim
    white, its presses do not count, and the others can still buzz. Switch
    them on again during a fastest-finger question: their buzzer stays dim
    white and cannot buzz until the next question, as the others' timers
    count from the start. In an everyone-answers question, it turns blue
    and can answer.
17. Fastest-finger question with no answers (the keys stand for the big
    button): buzz first with a keyboard key (say `5`).
    Expected: the buzzers turn dim white while it is judged. Judge it wrong:
    they turn blue again, and the first to press then counts.
18. **Late teams.** Between questions (a billboard, or a closed question),
    press a buzzer that is not in the show yet. Expected: it registers.
    Do the same during an everyone-answers question and during a
    fastest-finger question, and note whether its light and its press
    behaved as for the others.
19. **Spare buzzers.** Switch on a buzzer that is not in the show during a
    fastest-finger question: note its colour. It lights blue with the
    others and its press registers it as a late team.
20. **Idle buzzers.** Leave one buzzer untouched for ten minutes during a
    show, and switch another on mid-show. Note whether each kept or took
    its colour, went to sleep, and whether its first press counted.
21. End the show. Expected: every buzzer goes dark. Start a show again:
    in the lobby, the buzzers of teams already known turn blue. Do a test
    run from the editor (F5) and end it, then start a real show: no buzzer
    keeps a colour from the test run.

## 7. Failures

1. Mid-question, pull the USB cable. Expected: the Director shows "connection
   lost, reconnecting". Plug it back: "Enjoy 303S" again (not "does not
   answer yet") within about two seconds, and presses work without
   restarting the show. With the Enjoy
   303S the buzzers show their lights again once it is back: blue in a
   fastest-finger round, the results if they were showing, white and dim
   red while a buzz is judged. Also pull it while a buzz is being judged:
   once back, no buzz counts until the quizmaster judges.
2. Mid-question, pull the RJ45 plug at the receiver and plug it back,
   leaving the USB side in. The computer cannot see this, as the cable
   stays connected. Note whether presses and lights work again by
   themselves. If they do not, click **Reconnect** next to the receiver in
   the Receiver tab: presses and lights must come back as in step 1. Do the
   same with the receiver's power adapter, out for about five seconds,
   once while a question runs and once while a buzz is judged; note what
   the status, the lights and the presses did, and anything under "Raw
   data" when it powered up again.
3. **Reconnect while working.** During a fastest-finger question, with
   nothing unplugged, click **Reconnect**. The app sets up a receiver that
   is already open. Expected: "Enjoy 303S" again within a second. Note
   whether any light blinked or changed, and the timer of a press made
   right after (small: the timers started again).
4. Mid-question, force-quit the app. Start it again and resume the show:
   scores and the question state are as before, and the buzzers show the
   lights of the resumed step.

## 8. Release smoke check

The pages run under the app's content security policy only in an installed
build (`tauri dev` serves them without it), so check once on the Windows
installer, then on the Mac, that nothing it should allow is blocked:

1. Start a show with a quiz that has a picture, a sound and a video, under
   a built-in theme with its own fonts and background (Bakchus, say).
   Expected on the projector: the theme's fonts and background, the
   picture, the sound and the video, which plays to its end.
2. In the editor, select a slide with a sound and choose **Normalize
   volume**. Expected: it measures and reports the gain, with no error.
3. Choose something that asks first (ending the show, removing a file from
   the media library). Expected: the question appears with Yes and No in
   the app's language.
4. In the **Media player** tab, add a sound and a video and play each.
   Expected: both play.

A step that fails here and not in a dev build is the policy blocking it:
note the step and attach `buzzdome.log`, where each window notes what the
policy blocked ("the content security policy blocked …", at most five a
minute).

## 9. Send back

Attach to the project thread:

- the VID:PID, usage and interface of the receiver row;
- with the Enjoy 303S: the notes from sections 3, 6 and 7, above all the
  big button, other buttons, the colour byte of a press, the timer
  readings, the two-at-once presses, how the lights looked, and what the
  receiver did when its power or RJ45 plug went;
- the exported log files from sections 4 and 5;
- the result of the release smoke check (section 8);
- what failed, with the exact message.

For the Enjoy 303S that settles the open points of its protocol
(specification section 11). For any other receiver, the log is what a
protocol for it would be written from (specification BZ-5, BZ-10).

## Without the app

The same raw reports can be printed from a terminal on any machine with Rust:

```sh
cargo run -p buzzers --example hid-monitor                      # list devices
cargo run -p buzzers --example hid-monitor -- 1234:abcd         # print reports
cargo run -p buzzers --example hid-monitor -- 10c4:1819 rush    # Enjoy 303S, fastest finger
cargo run -p buzzers --example hid-monitor -- 10c4:1819 lights  # Enjoy 303S, every light
```

It runs the app's own receiver code: it prints the status (for the Enjoy
303S, `answered: true` once the receiver answered its set-up), each report
and the presses read from it, with each buzzer's timer. The buzzers show
blue and take answers A to D; with `rush` they are in a fastest-finger
round, whose presses come in the order of their timers. With `lights` every
buzzer shows off, ready, locked, first, right and wrong in turn, two seconds
each, and a second into each, buzzers 2 and 6 alone take the next light.
Every press is taken then: on a buzzer other than 2 and 6, one made just
before the light changes shows a timer near 2000 if the timer counts
milliseconds.
