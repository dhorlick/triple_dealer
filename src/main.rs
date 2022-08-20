extern crate midir;

use std::io::{stdin, stdout, Write};
use std::error::Error;

use std::collections::HashMap;
use std::time::SystemTime;
use std::env;

use midir::{MidiInput, MidiOutput, MidiIO, Ignore};

const NOTE_ON_MESSAGE_TYPE: u8 = 9;
const NOTE_OFF_MESSAGE_TYPE: u8 = 8;
const PROGRAM_CHANGE_MESSAGE_TYPE: u8 = 12;
const PITCH_BEND_MESSAGE_TYPE: u8 = 14;

fn main()
{
    let args: Vec<String> = env::args().collect();
    
    match run(!args.contains(&("--no_multicast".to_string())), !args.contains(&("--no_log".to_string())))
    {
        Ok(_) => (),
        Err(err) => panic!("{}", err)
    }
}

#[derive(Copy, Clone, Debug)]
struct Tone
{
    pub original_midi_channel: u8,
    pub assigned_midi_channel: u8,
    pub started: SystemTime,
}

fn describe_single_data_midi_message(stamp: u64, midi_message_type: u8, midi_channel: u8, midi_note_index_or_program: u8)
{
    if midi_message_type == NOTE_ON_MESSAGE_TYPE
    {
        println!("time: {}, type: Note On, channel: {}, tone: {}", stamp, midi_channel, midi_note_index_or_program);
    }
    else if midi_message_type == NOTE_OFF_MESSAGE_TYPE
    {
        println!("time: {}, type: Note Off, channel: {}, tone: {}", stamp, midi_channel, midi_note_index_or_program);
    }
    else if midi_message_type == PROGRAM_CHANGE_MESSAGE_TYPE
    {
        println!("time: {}, type: Program Change, channel: {}, program: {}", stamp, midi_channel, midi_note_index_or_program);
    }
    else 
    {
        println!("time: {}, type: {}, channel: {}, data1: {}", stamp, midi_message_type, midi_channel, midi_note_index_or_program);
    }
}

fn describe_double_data_midi_message(stamp: u64, midi_message_type: u8, midi_channel: u8, data1: u8, data2: u8)
{
    if midi_message_type == PITCH_BEND_MESSAGE_TYPE
    {
        println!("time: {}, type: Pitch Bend, channel: {}, Range (LSB): {}, Range (MSB): {}", stamp, midi_channel, data1, data2);
    }
    else if midi_message_type == NOTE_ON_MESSAGE_TYPE
    {
        println!("time: {}, type: Note On, channel: {}, tone: {}, velocity: {}", stamp, midi_channel, data1, data2);
    }
    else if midi_message_type == NOTE_OFF_MESSAGE_TYPE
    {
        println!("time: {}, type: Note Off, channel: {}, tone: {}, velocity: {}", stamp, midi_channel, data1, data2);
    }
    else
    {
        println!("time: {}, type: {}, channel: {}, data1: {}, data2: {}", stamp, midi_message_type, midi_channel, data1, data2);
    }
}

fn describe_double_data_midi_message_to_from(stamp: u64, midi_message_type: u8, midi_channel_from: u8, midi_channel_to: u8, midi_note_index: u8, velocity: u8)
{
    if midi_message_type == NOTE_ON_MESSAGE_TYPE
    {
        println!("time: {}, type: Note On, channel: {} -> {}, tone: {}, velocity: {}", stamp, midi_channel_from, midi_channel_to, midi_note_index, velocity);
    }
    else if midi_message_type == NOTE_OFF_MESSAGE_TYPE
    {
        println!("time: {}, type: Note Off, channel: {} -> {}, tone: {}, velocity: {}", stamp, midi_channel_from, midi_channel_to, midi_note_index, velocity);
    }
    else
    {
        println!("time: {}, type: {}, channel: {} -> {}, tone: {}, velocity: {}", stamp, midi_message_type, midi_channel_from, midi_channel_to, midi_note_index, velocity);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run(relay_channelwide_events_to_all_channels: bool, echo_to_console: bool) -> Result<(), Box<dyn Error>>
{
    let mut midi_in = MidiInput::new("midi_splitter input")?;
    midi_in.ignore(Ignore::None);
    let midi_out = MidiOutput::new("midi_splitter output")?;

    let in_port = select_port(&midi_in, "input")?;
    println!();
    let out_port = select_port(&midi_out, "output")?;

    println!("\nOpening connections");
    let in_port_name = midi_in.port_name(&in_port)?;
    let out_port_name = midi_out.port_name(&out_port)?;

    let mut conn_out = midi_out.connect(&out_port, "midi_splitter")?;

    let mut by_midi_note: HashMap<u8, Tone> = HashMap::new();  // TODO add original channel in here, too?
    let mut by_destination_channel: [Option<Tone>; 3] = [None; 3];

    let _conn_in = midi_in.connect(&in_port, "midi_splitter", move |stamp, message: &[u8], _|
    {
        let status = message[0];
        let midi_message_type = status >> 4;
        let midi_channel: u8 = status & 0xF;

        if midi_message_type == NOTE_OFF_MESSAGE_TYPE || midi_message_type == NOTE_ON_MESSAGE_TYPE
        {
            // split-out note to a new midi channel if necessary
            
            let midi_note_index: u8 = message[1];
            let velocity: u8 = message[2];
            
            let active = midi_message_type == NOTE_ON_MESSAGE_TYPE;
            if active
            {
                match by_midi_note.get(&midi_note_index)
                {
                    Some(preexisting_tone) => 
                    {
                        // note already in progress. touch and move on
                        by_midi_note.insert(midi_note_index, Tone 
                        {
                            original_midi_channel: preexisting_tone.original_midi_channel,
                            assigned_midi_channel: preexisting_tone.assigned_midi_channel,
                            started: SystemTime::now()
                        });
                    }
                    _ => 
                    {
                        // (re-)assign and relay
                        
                        let assigned_midi_channel: u8 = match by_destination_channel.iter()
                                .position(|&r| r.is_none())
                        {
                            Some(available_midi_channel) => 
                            {
                                available_midi_channel as u8
                            }
                            _ =>
                            {
                                let index_of_oldest: usize = by_destination_channel.iter()
                                        .enumerate()
                                        .min_by_key(|x| x.1.unwrap().started)
                                        .map(|(index, _)| index).unwrap();
                                
                                // send note-off for the eclipsed note
                                let assigned_midi_channel: u8 = index_of_oldest as u8;
                                let new_status: u8 = (NOTE_OFF_MESSAGE_TYPE << 4) | assigned_midi_channel;
                                let new_message = [new_status, midi_note_index, velocity];
                                conn_out.send(&new_message).unwrap_or_else(|_| println!("Error when forwarding message ..."));
                                if echo_to_console
                                {
                                    describe_double_data_midi_message_to_from(stamp, midi_message_type, midi_channel, assigned_midi_channel, midi_note_index, velocity);
                                }
                                assigned_midi_channel
                            }
                        };

                        let tone = Tone 
                        {
                            original_midi_channel: midi_channel,
                            assigned_midi_channel: assigned_midi_channel,
                            started: SystemTime::now()
                        };
                        by_midi_note.insert(midi_note_index, tone);
                        by_destination_channel[usize::from(assigned_midi_channel)] = Some(tone); 

                        let new_status = (midi_message_type << 4) | assigned_midi_channel;
                        let new_message = [new_status, midi_note_index, velocity];
                        conn_out.send(&new_message).unwrap_or_else(|_| println!("Error when forwarding message ..."));
                        if echo_to_console
                        {
                            describe_double_data_midi_message_to_from(stamp, midi_message_type, midi_channel, assigned_midi_channel, midi_note_index, velocity);
                        }
                    }
                }
            }
            else
            {
                match by_midi_note.get(&midi_note_index)
                {
                    Some(preexisting_tone) => 
                    {
                        let assigned_midi_channel = preexisting_tone.assigned_midi_channel;
                        by_destination_channel[usize::from(assigned_midi_channel)] = None;
                        by_midi_note.remove(&midi_note_index);
                        let status = (midi_message_type << 4) | assigned_midi_channel;
                        let message = [status, midi_note_index, velocity];
                        conn_out.send(&message).unwrap_or_else(|_| println!("Error when forwarding message ..."));
                        if echo_to_console
                        {
                            describe_double_data_midi_message_to_from(stamp, midi_message_type, midi_channel, assigned_midi_channel, midi_note_index, velocity);
                        }
                    }
                    _ => 
                    {
                        println!("no cached note to shutdown for midi_note_index {}:", midi_note_index);
                                // possibly because we hit the 3-note polyphony limit and had to stop it earlier
                    }
                }
            }
        }
        else if relay_channelwide_events_to_all_channels && (midi_message_type == PROGRAM_CHANGE_MESSAGE_TYPE || midi_message_type == PITCH_BEND_MESSAGE_TYPE) 
        {
            let data1: u8 = message[1];
            for destination_midi_channel in 0..=2
            {
                let status = (midi_message_type << 4) | destination_midi_channel;
                if midi_message_type == PROGRAM_CHANGE_MESSAGE_TYPE
                {
                    let message = [status, data1, 0];
                    conn_out.send(&message).unwrap_or_else(|_| println!("Error when forwarding message ..."));
                    if echo_to_console
                    {
                        describe_single_data_midi_message(stamp, midi_message_type, destination_midi_channel, data1);
                    }
                }
                else if midi_message_type == PITCH_BEND_MESSAGE_TYPE
                {
                    let data2 = message[2];
                    let message = [status, data1, data2];
                    conn_out.send(&message).unwrap_or_else(|_| println!("Error when forwarding message ..."));
                    if echo_to_console
                    {
                        describe_double_data_midi_message(stamp, midi_message_type, destination_midi_channel, data1, data2);
                    }
                }
                else
                {
                    panic!("Bug: unsupported midi_message_type: {}", midi_message_type);
                }
            }
        }
        else
        {
            conn_out.send(message).unwrap_or_else(|_| println!("Error when forwarding message ..."));
            if echo_to_console
            {
                println!("{}: {:?}", stamp, message);
            }
        }
    }, ())?;

    println!("Connections open, {} -> {}. Press Control-C to exit.", in_port_name, out_port_name);

    loop {}
}

fn select_port<T: MidiIO>(midi_io: &T, descr: &str) -> Result<T::Port, Box<dyn Error>> {
    println!("Available {} ports:", descr);
    let midi_ports = midi_io.ports();
    for (i, p) in midi_ports.iter().enumerate() {
        println!("{}: {}", i, midi_io.port_name(p)?);
    }
    print!("Please select {} port: ", descr);
    stdout().flush()?;
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    let port = midi_ports.get(input.trim().parse::<usize>()?)
                         .ok_or("Invalid port number")?;
    Ok(port.clone())
}

#[cfg(target_arch = "wasm32")]
fn run() -> Result<(), Box<dyn Error>> {
    println!("test_forward cannot run on Web MIDI");
    Ok(())
}