# Hardware: wireless buzzers

Research notes, last updated 2026-10-09. Nothing here is verified on real
hardware yet. The driver built from these notes is described in
`docs/design.md`; the test plan for the real receiver is
`docs/hardware-test.md`. Section numbers point into the v8.5 manual
(`replica/sources/manual-v8.5/`).

## What the events use

Players answer only with physical wireless buzzers, no phones. A colleague owns
QuizXpress-branded buzzers, one of these two models (not yet confirmed which):

| | Dome Buzzer | Galaxy RGB Buzzer |
| --- | --- | --- |
| buttons | A-D + big fastest-finger button | A-D + big fastest-finger button |
| light | fixed states: green = ready, red = locked out, white flashing = first to buzz | RGB, 64 colours: ready, correct, incorrect, chosen answer, first to buzz, light shows, lucky-winner draw |
| power | 2 x D batteries | rechargeable, power adapter |
| per receiver | up to 100 | up to 64 |
| ID programming | B + big button | C + big button |
| likely OEM model | Enjoy B215 ("rf215") | Enjoy 303S / B303 |

How to tell them apart: a battery compartment with two D cells means Dome; a
charging socket means Galaxy. QuizXpress keypads (6 answers + fastest finger +
"hand" button, 400 per receiver) match the Enjoy RF217 keypad.

## Manufacturer (high confidence)

QuizXpress never names its supplier, but the evidence points one way:

- **Enjoy Technology Co., Ltd** (Hong Kong), FCC grantee code `RJM`; the
  manufacturer named in its 2022 FCC test report is **Guangzhou Qile
  Technology Co., Ltd** (brand 齐乐 / 齐齐乐, site qiletech.cn). English site:
  enjoyvote.com. US distributor: gameshowsandbuzzers.com.
- Same product photos, light behaviour, ID-programming key combinations,
  limits (100 buzzers / 400 keypads / 64 RGB buzzers per receiver), standalone
  remote button set, and the "Galaxy" name for LED wristbands.
- Names of public third-party files around QuizXpress tooling reference Enjoy
  and RF215 components (see "Deliberately not used" below).

## What is known about the link

Receiver (Enjoy REC-A "half ball", FCC ID RJM-ENGAGE, 2013 user manual exhibit):

- 137 x 137 x 50 mm, RJ45 socket, LEDs for power, PC communication, activity.
- NXP P89LV51RD2 (8051) MCU, 24Cxx EEPROM, four TI CC2500 2.4 GHz radio
  modules working in parallel, SN75179 differential driver/receiver on the
  RJ45 line. GFSK, 1 Mbps, channels 2402-2480 MHz; 4 selectable radio channels.
- Powered from the PC through the cable. The PC link is a differential serial
  line; the USB side lives in the cable.

Cable ("USB-HID cable", USB to RJ45, inline box labelled "HID USB CABLE"):

- Windows needs no driver: a USB HID device that tunnels the receiver's serial
  link ("serial port with HID device" in the vendor SDK's own description).
  Enjoy also sells driver-based variants (USB-serial) and an RS-232 port.
- Chip and USB VID/PID unknown. If it is a Silicon Labs CP2110 (10C4:EA80),
  its HID-to-UART interface is publicly documented (AN434) and Linux has a
  `hid-cp2110` driver.

Behaviour:

- Two-way: the PC sets buzzer lights (ready, locked out, first, correct,
  wrong; colours on RGB models) and can read battery level (QuizXpress 8.6
  added a dome-buzzer battery test).
- Buzzers are paired to one receiver and carry a programmable ID; QuizXpress
  sells "cloned" spare receivers built from the label under the original, so
  each receiver has its own identity.
- Only one application can open the receiver at a time.
- Enjoy claims "press time precise to 1 ms" and "100 buzzers polled in 0.5 s".
  The vendor SDK's public header text mentions per-press timestamps only for a
  newer model (RF317). Whether the RF215 receiver reports press times or just
  the arrival order is an open question, and it decides how fair our
  fastest-finger ordering can be.

Vendor software model (from public descriptions of Enjoy's SDK "librf21x"):

- C/C++ SDK for rf215 / rf217 / rf218 / rf219 / rf228, Windows, Linux (libusb)
  and macOS (HID). Connection strings for serial, HID cable and TCP.
- Operations: open with an ID range, start/stop a quiz in a mode (rush =
  fastest finger, single, multiple), read messages (host remote, player
  button, set-ID result, receiver channel / power / serial / version), set a
  keypad ID, and raw read/write on the serial link.
- Enjoy supplies SDKs (.NET, Java, C#, keystroke) on request only.
- Most third-party quiz software (Crowd Control Games, GameShow Presenter,
  Next Level Game Shows) uses Enjoy's "Keystroke" program instead: buzzer
  presses become key presses, with no light control and no lockout. The French
  iZiMiX / Enjoy Slam drives lights and lockout natively.

## Manufacturer's answers (October 2026)

Enjoy Tech replied to Jakub's email:

- The model is either 215 or 303S; check the label under the base.
- The protocol is described in their SDK.
- The receiver reports the press **time** (so fastest-finger ranking can use
  hardware times, not just arrival order).
- Different versions use different protocols (dome and RGB differ).
- The SDK is free of charge ("not licence charged"); the reference app is the
  paid part. They asked why we want it open source and which part. What the
  SDK terms allow (implementing the protocol in this app, publishing that
  code) is still unanswered.

## How we get the protocol

1. **Identify.** Photos of the receiver, the cable and the label under a
   buzzer (requested from the colleague). Windows Device Manager > receiver >
   Properties > Details > Hardware Ids gives the USB VID/PID; on a Mac,
   `system_profiler SPUSBDataType`.
2. **Manufacturer (email sent 2026-10-05).** Follow up by asking for the
   librf21x SDK and protocol documentation for the RF215 receiver with HID
   cable, and for terms that allow an open-source app to either implement the
   protocol or load the vendor library.
3. **Capture.** If the cable is a HID-UART bridge, capture on a Windows PC with
   QuizXpress (Wireshark + USBPcap) and decode the serial frames inside the HID
   reports: connect, every button on two or three buzzers, a fastest-finger
   race, each light state, RGB colours, battery test, programming an ID.
   Captures go in `replica/captures/` (gitignored).
4. **Spike in Rust.** Done: `cargo run -p buzzers --example hid-monitor`
   prints raw reports with timestamps, and the app's Receiver tab records and
   exports them. Lights follow once the protocol is known.

## Design consequences

- A `BuzzerDriver` trait in Rust with backends: native protocol (target),
  vendor library through FFI as an optional user-installed plugin if its
  licence allows (never bundled without permission), keyboard (keys 1 to 0,
  also covers Enjoy Keystroke), and on-screen virtual buzzers for testing.
- Fastest-finger fairness: trust the receiver's order or timestamps; the PC
  never re-orders presses. Document this once the captures show what the
  receiver reports.

## Deliberately not used

Public GitHub repositories exist that contain (a) a third-party repost of
Enjoy's SDK without a licence and (b) decompiled QuizXpress / Enjoy
programming tools. Both are proprietary code. To keep the project clean-room
and publishable, nobody on this project reads or copies their code; only the
public README-level facts above were noted. Get the SDK from Enjoy directly.

## Sources

| source | URL |
| --- | --- |
| QuizXpress manual 8.5, §4.1, §5.1, §8.1, Appendix A | `replica/sources/manual-v8.5/` |
| QuizXpress game show buzzers page | https://www.quizxpress.com/game-show-buzzers/ |
| QuizXpress FAQ (receiver LEDs, 2.4 GHz) | https://www.quizxpress.com/faq/ |
| QuizXpress shop: USB-HID cable for receiver | https://www.quizxpress.com/products/usb-hid-cable-for-quizxpress-receiver/ |
| QuizXpress standalone remote (PDF) | https://www.quizxpress.com/wp-content/uploads/2021/04/QuizXpress-Standalone-remote.pdf |
| QuizXpress forum: programming keypads and buzzers | https://forum.quizxpress.com/viewtopic.php?t=69 |
| QuizXpress forum: user asking for a Dome buzzer SDK (no answer) | https://forum.quizxpress.com/viewtopic.php?t=88 |
| QuizXpress 8.0 release notes | https://forum.quizxpress.com/viewtopic.php?p=156 |
| Enjoy Tech dome buzzer and REC-A receiver | https://www.enjoyvote.com/en-us/products.php?view=75 |
| Enjoy Tech 303S RGB buzzer | https://www.enjoyvote.com/en-us/products.php?view=97 |
| Enjoy Tech integration kit (SDK / API) | https://www.enjoyvote.com/en-us/products.php?view=60 |
| Enjoy Tech downloads (HID versions need no driver) | https://www.enjoyvote.com/en-us/downloads.php |
| Qile Technology, Chinese site | http://www.qiletech.cn/cn/ |
| FCC grantee RJM (Enjoy Technology) | https://fccid.io/RJM |
| FCC RJM-ENGAGE receiver user manual exhibit | https://fccid.io/pdf.php?id=1912229 |
| Enjoy distributor: buzzers, receivers | https://www.gameshowsandbuzzers.com/buzzers.html, https://www.gameshowsandbuzzers.com/receivers.html |
| iZiMiX manual (Enjoy B215 / B303 behaviour) | https://bongcommunication.com/wp-content/uploads/2023/12/iZiMiX-Blind-Test-Machine-rtfm-v617.pdf |
