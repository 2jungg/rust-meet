use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Sample, SampleFormat, Stream, StreamConfig,
};
use std::error::Error;
use tokio::sync::mpsc;

pub fn setup_audio_streams(
    audio_sender: mpsc::UnboundedSender<Vec<f32>>,
    audio_receiver: mpsc::UnboundedReceiver<Vec<f32>>,
) -> Result<(Stream, Stream), Box<dyn Error>> {
    let host = cpal::default_host();

    // Input stream
    let input_device = host
        .default_input_device()
        .ok_or("No input device available")?;
    let input_config = input_device.default_input_config()?;
    let input_stream = match input_config.sample_format() {
        SampleFormat::F32 => {
            create_input_stream::<f32>(&input_device, &input_config.into(), audio_sender)
        }
        _ => Err("Unsupported sample format".into()),
    }?;

    // Output stream
    let output_device = host
        .default_output_device()
        .ok_or("No output device available")?;
    let output_config = output_device.default_output_config()?;
    let output_stream = match output_config.sample_format() {
        SampleFormat::F32 => {
            create_output_stream::<f32>(&output_device, &output_config.into(), audio_receiver)
        }
        _ => Err("Unsupported sample format".into()),
    }?;

    input_stream.play()?;
    output_stream.play()?;

    Ok((input_stream, output_stream))
}

fn create_input_stream<T>(
    device: &Device,
    config: &StreamConfig,
    sender: mpsc::UnboundedSender<Vec<f32>>,
) -> Result<Stream, Box<dyn Error>>
where
    T: Sample + cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            let samples: Vec<f32> = data.iter().map(|s| s.to_sample::<f32>()).collect();
            if sender.send(samples).is_err() {
                // eprintln!("Failed to send audio data");
            }
        },
        |err| eprintln!("An error occurred on the input audio stream: {}", err),
        None
    )?;
    Ok(stream)
}

fn create_output_stream<T>(
    device: &Device,
    config: &StreamConfig,
    mut receiver: mpsc::UnboundedReceiver<Vec<f32>>,
) -> Result<Stream, Box<dyn Error>>
where
    T: Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            if let Ok(audio_data) = receiver.try_recv() {
                let len = std::cmp::min(data.len(), audio_data.len());
                for (i, sample) in data.iter_mut().enumerate().take(len) {
                    *sample = T::from_sample(audio_data[i]);
                }
            } else {
                // Fill with silence if no data
                for sample in data.iter_mut() {
                    *sample = T::from_sample(0.0);
                }
            }
        },
        |err| eprintln!("An error occurred on the output audio stream: {}", err),
        None
    )?;
    Ok(stream)
}
