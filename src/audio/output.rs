use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};

/// Streaming audio output backed by cpal.
///
/// `push_samples` accepts interleaved stereo f32 frames. The backend converts
/// them to the device sample format and channel count.
pub struct AudioOutput {
    _stream: cpal::Stream,
    tx: SyncSender<Vec<f32>>,
    sample_rate: u32,
}

impl AudioOutput {
    pub fn new() -> Option<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        let supported = device.default_output_config().ok()?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.config();
        let sample_rate = config.sample_rate;
        let (tx, rx) = sync_channel::<Vec<f32>>(8);
        let err_fn = |err| eprintln!("audio output error: {err}");

        let stream = match sample_format {
            cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config, rx, err_fn),
            cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config, rx, err_fn),
            cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config, rx, err_fn),
            _ => None,
        }?;

        stream.play().ok()?;
        Some(Self {
            _stream: stream,
            tx,
            sample_rate,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn push_samples(&mut self, samples: &[f32]) {
        let _ = self.tx.try_send(samples.to_vec());
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    rx: Receiver<Vec<f32>>,
    err_fn: impl FnMut(cpal::Error) + Send + 'static,
) -> Option<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut current: Vec<f32> = Vec::new();
    let mut frame = 0usize;

    device
        .build_output_stream(
            *config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                for out_frame in data.chunks_mut(channels) {
                    if current.is_empty() || frame * 2 + 1 >= current.len() {
                        match rx.try_recv() {
                            Ok(chunk) => {
                                current = chunk;
                                frame = 0;
                            }
                            Err(_) => {
                                for sample in out_frame.iter_mut() {
                                    *sample = T::EQUILIBRIUM;
                                }
                                continue;
                            }
                        }
                    }

                    let left = current[frame * 2];
                    let right = current[frame * 2 + 1];
                    if channels == 1 {
                        out_frame[0] = T::from_sample((left + right) * 0.5);
                    } else {
                        out_frame[0] = T::from_sample(left);
                        out_frame[1] = T::from_sample(right);
                        for sample in out_frame.iter_mut().skip(2) {
                            *sample = T::EQUILIBRIUM;
                        }
                    }
                    frame += 1;
                }
            },
            err_fn,
            None,
        )
        .ok()
}
