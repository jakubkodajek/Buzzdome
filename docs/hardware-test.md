# Hardware test: buzzer receiver

A checklist for the first test with the real receiver and buzzers. It checks
specification section 4 and collects what is needed to write the vendor
protocol. Do it on the event laptop (Windows) first, then on a Mac. Takes
about 30 minutes with four buzzers.

## What to have

- The receiver, the USB-HID cable, at least four buzzers with batteries or
  charged, each with its own ID (the vendor's ID programming: hold B or C and
  the big button, see `replica/hardware.md`).
- The app installed from the latest CI run (GitHub > Actions > Build > the run
  > Artifacts: `quiz-show-windows`, `quiz-show-macos` or `quiz-show-linux`).
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
4. Click **Use**. Expected: status "Connected ... protocol raw" (green).
   - Linux: if it says the device cannot be opened, copy the udev rule from
     the tab, install it as shown, unplug and replug.
   - macOS: if asked about Input Monitoring, allow the app, then restart it.
   - Windows: if it cannot open, make sure no other quiz program is running.

## 3. Record raw reports

1. Press **A** on buzzer 1. Expected: a line appears under "Raw reports".
   - If nothing appears: try the other rows with the same VID:PID. If none
     shows reports, the receiver probably needs a start command from the
     vendor protocol; note it and go to step 5 (export).
2. On buzzer 1, press A, B, C, D and the big button, each twice, slowly.
   Repeat for buzzer 2 and buzzer 3.
3. Press the big button on buzzers 1 and 2 at the same moment, five times.
4. Hold the big button on buzzer 1 for three seconds.
5. Click **Export log** and save the file.

## 4. Teach buttons (learn mode)

1. Under "Teach buttons": buzzer 1, button A, **Start**. Press A three times,
   then **Save**. The form moves on to B, then C, D, the big button, then
   buzzer 2.
2. Teach buzzers 1 to 4.
3. Expected: pressing any taught button now shows "buzzer N, button X" under
   "Presses". If saving shows a conflict, note which buttons; export the log
   again.

## 5. Play

1. Start the demo quiz. In the lobby, press the big button on each buzzer:
   each registers as a team.
2. Everyone-answers question: each buzzer answers with A-D; the counts match.
3. Fastest-finger question: two players press together several times. The
   team shown first must match who the players think was first; note any
   case where it does not.
4. Hold a button: it must count once.
5. Keyboard keys `1`-`0` still work next to the buzzers.

## 6. Failures

1. Mid-question, pull the USB cable. Expected: the Director shows "connection
   lost, reconnecting". Plug it back: "Connected" again within about two
   seconds, and presses work without restarting the show.
2. Mid-question, force-quit the app. Start it again and resume the show:
   scores and the question state are as before.

## 7. Send back

Attach to the project thread:

- the model (215 or 303S) and the photos;
- the VID:PID, usage and interface of the receiver row;
- the exported log files from steps 3 and 4;
- what failed, with the exact message.

With the log and the vendor SDK, the vendor protocol can be written so that
lights, press times and battery level work (specification BZ-5, BZ-10, BZ-14).

## Without the app

The same raw reports can be printed from a terminal on any machine with Rust:

```sh
cargo run -p buzzers --example hid-monitor              # list devices
cargo run -p buzzers --example hid-monitor -- 1234:abcd # print reports
```
