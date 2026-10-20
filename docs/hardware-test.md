# Hardware test: buzzer receiver

A checklist for the first test with the real receiver and buzzers. It checks
specification section 4 and collects what is needed to write the vendor
protocol, then smoke-tests the installed build. Do it on the event laptop (Windows) first, then on a Mac. Takes
about 30 minutes with four buzzers.

## What to have

- The receiver, the USB-HID cable, at least four buzzers with batteries or
  charged, each with its own ID (the vendor's ID programming: hold B or C and
  the big button, see `replica/hardware.md`).
- The app installed from the latest CI run (GitHub > Actions > Build > the run
  > Artifacts: `buzzdome-windows`, `buzzdome-macos` or `buzzdome-linux`).
- On Windows, a build from before the rename to Buzzdome uninstalled first:
  Settings > Apps > **Quiz Show** > Uninstall, with "Delete the application
  data" left unticked, as Buzzdome keeps its quizzes, teams and settings in
  that same folder. The installer tells programs apart by name, so Buzzdome
  installs beside Quiz Show rather than over it, and both would write the
  same show journal. Buzzdome itself runs once (starting it again brings the
  running Director forward), but an old Quiz Show does not step aside.
- The vendor's quiz software closed: only one program can open the receiver.

## 1. Identify

1. Read the label under a buzzer base: model 215 (dome) or 303S (RGB).
   Note it, and take a photo of the label, the receiver and the cable's
   inline box.
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
   - **Enjoy RF215** for VID:PID `10c4:1819` once the receiver answered:
     go on with section 3 (learn mode is not offered, section 5 does not
     apply).
   - **Enjoy RF215, but the receiver did not answer the connect command**:
     the cable is the RF215's, the receiver is not. Check its power light
     and the RJ45 plug, then unplug and plug the cable again. If it stays,
     note it and go on with section 3 all the same.
   - **raw reports only** for any other receiver: skip section 3.
   - Linux: if it says the device cannot be opened, copy the udev rule from
     the tab, install it as shown, unplug and replug.
   - macOS: if asked about Input Monitoring, allow the app, then restart it.
   - Windows: if it cannot open, make sure no other quiz program is running.

## 3. Enjoy RF215

Only when the status names the Enjoy RF215. The app sets the receiver up by
itself, as described in `docs/design.md` (2.4); these steps check what could
not be checked without the hardware. Keep the Receiver tab open: "Presses"
shows what the app read, "Raw data" the bytes behind it.

1. With no show running, the buzzers take answers. Press A, B, C and D on
   buzzer 1, then on buzzers 2 to 4. Expected: "buzzer N, button X" under
   "Presses" for each, with the buzzer's own number.
   - Raw data but no presses, or the wrong buttons: note which, then go on
     with step 5 of section 4 (export).
   - Nothing at all: check the label (section 1). A 303S speaks another
     protocol, which the app does not have yet.
2. Press the big button on buzzer 1 and note whether it shows as a press
   (the receiver may report only A to D while everyone answers; teams
   register with A to D either way).
3. **Buzzer numbers above 15.** This is the part the app has to guess. Press
   A on buzzers numbered 16 or more (give one such a number with the
   vendor's ID programming if none has one). Expected: their own numbers.
   If they show other numbers (say 0 or 4 for 16 or 20), or only a line
   under "Raw data" and no press (the app drops numbers above 100), note
   each real number and what the app showed, and export the log.
4. Section 6 (play) then checks the lights and the rounds.

## 4. Record raw reports

1. Press **A** on buzzer 1. Expected: a line appears under "Raw reports".
   - If nothing appears: try the other rows with the same VID:PID. If none
     shows reports, the receiver probably needs a start command from the
     vendor protocol; note it and go to step 5 (export).
2. On buzzer 1, press A, B, C, D and the big button, each twice, slowly.
   Repeat for buzzer 2 and buzzer 3.
3. Press the big button on buzzers 1 and 2 at the same moment, five times.
4. Hold the big button on buzzer 1 for three seconds.
5. Click **Export log** and save the file.

## 5. Teach buttons (learn mode)

Not with the Enjoy RF215, whose protocol the app knows: the section is
hidden then.

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

With the Enjoy RF215 also:

6. Fastest-finger question: when it starts, the buzzers light green
   (ready); when one presses, it flashes white and beeps and the others
   turn red (the receiver does this by itself). Note what each light did.
7. Judge that buzz wrong. Expected: that buzzer shows wrong, then the
   others can buzz again. Note whether the one judged wrong can buzz
   again too (it should not).
8. Judge a buzz right. Expected: that buzzer shows right.
9. Everyone-answers question: after the results, the buzzers that answered
   right show right and the others wrong. Note if nothing lights.
10. Switch one player off in the Director and press their big button at
    the start of a fastest-finger question. Expected: the others can still
    buzz.
11. Fastest-finger question: buzz first with a keyboard key (say `5`).
    Expected: the domes go dark while it is judged, and none flashes as
    first when pressed. Judge it wrong: all domes light as ready again, and
    the first to press then flashes and counts. If a dome judged wrong
    earlier in that question presses first, it may flash and beep once;
    the others must still be able to buzz right after, and it must not
    count.

## 7. Failures

1. Mid-question, pull the USB cable. Expected: the Director shows "connection
   lost, reconnecting". Plug it back: "Connected" again within about two
   seconds, and presses work without restarting the show. With the Enjoy
   RF215 the domes show the round again (a fastest-finger round begins
   anew), and the results if they were showing. Also pull it while a buzz
   is being judged: once back, no dome flashes as first until the
   quizmaster judges.
2. Mid-question, force-quit the app. Start it again and resume the show:
   scores and the question state are as before.

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

- the model (215 or 303S) and the photos;
- the VID:PID, usage and interface of the receiver row;
- with the Enjoy RF215: the notes from sections 3 and 6, above all the
  buzzer numbers above 15;
- the exported log files from sections 4 and 5;
- the result of the release smoke check (section 8);
- what failed, with the exact message.

For the Enjoy RF215 that settles the open points of its protocol. For any
other receiver, the log and the vendor SDK are what the vendor protocol is
written from, so that lights, press times and battery level work
(specification BZ-5, BZ-10, BZ-14).

## Without the app

The same raw reports can be printed from a terminal on any machine with Rust:

```sh
cargo run -p buzzers --example hid-monitor                    # list devices
cargo run -p buzzers --example hid-monitor -- 1234:abcd       # print reports
cargo run -p buzzers --example hid-monitor -- 10c4:1819 rush  # Enjoy RF215, big button
```

It runs the app's own receiver code: it prints the status (for the Enjoy
RF215, `answered: true` once the receiver answered), each report and the
presses read from it. An RF215 takes answers A to D, or with `rush` a
fastest-finger round, which the monitor sets going again after each buzz:
a buzzer that buzzed once should then stay out, which is worth noting too.
