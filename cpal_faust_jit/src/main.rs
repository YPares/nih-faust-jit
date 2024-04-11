use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("No output device available")?;
    // let mut supported_configs_range = device
    //     .supported_output_configs();
    println!("Output device: {}", device.name()?);
    //let def_input_cfg = device.default_input_config()?;
    //println!("{:?}", def_input_cfg);
    let def_output_cfg = device.default_output_config()?.into();
    let stream = device.build_output_stream(
        &cpal::StreamConfig {
            buffer_size: cpal::BufferSize::Fixed(256),
            ..def_output_cfg
        },
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            println!("{}", data.len());
            // react to stream events and read or write stream data here.
        },
        move |_err| {
            // react to errors here.
        },
        None, // None=blocking, Some(Duration)=timeout
    )?;
    stream.play().unwrap();

    std::thread::sleep(std::time::Duration::from_secs(1));

    Ok(())
}
