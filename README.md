Takes up to three concurrent MIDI notes input on the same (or any) input MIDI channel and distributes them to output channels 1, 2 or 3 (0, 1, or 2, from the computer's perspective). This is useful for achieving polyphony (for example, chords) on devices running [trash80/Ym2149Synth](https://github.com/trash80/Ym2149Synth) driven by a MIDI keyboard.

Ordinarily, program changes and pitch wheel Adjustments will be redundantly transcribed to all three destination channels. (See also Command Line Arguments, below.)

If more than three concurrent notes are input, an implicit Note Off message will be sent to drop the oldest.

# Installation

   1. [Install Rust and Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html).
   1. Plug in your MIDI keyboard and the target MIDI device, for example a [Catskull Electronics YM2149 Synth](https://catskullelectronics.com/collections/synths/products/ym2149-synth?variant=40960685342908).
   1. Clone this repository and change into the resulting directory.
   1. type `cargo run` or `cargo build && ./target/debug/triple_dealer`

# Operation

At startup, type the numbers corresponding to your MIDI selections, pressing enter after each.

To quit once triple_dealer has started, press Control-C.

Polyphony and the pitch wheel work better with some Ym2149Synth programs/voices than others. The first program (Square-voice) works well with both.

## Command line arguments

Use a `--no_log` argument to refrain from logging MIDI events to the console.

Use a `--no_multicast` argument to refrain from redundantly transcribing events to all three destination channels. Instead, these will go to whichever channel is specified by the input MIDI message.

Note that the syntax for providing command line arguments in conjunction with `cargo run` is kind of weird. Here's an example of how to do it:

```
cargo run -- --no_log
```