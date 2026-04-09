use rodio::{OutputStream, Sink, Source};
use std::time::Duration;

/// Play a short rising tone to indicate recording started.
pub fn play_start_sound() {
    std::thread::spawn(|| {
        if let Ok((_stream, handle)) = OutputStream::try_default() {
            if let Ok(sink) = Sink::try_new(&handle) {
                // Rising two-tone beep: 600Hz then 800Hz
                let tone1 = rodio::source::SineWave::new(600.0)
                    .take_duration(Duration::from_millis(80))
                    .amplify(0.08);
                let tone2 = rodio::source::SineWave::new(800.0)
                    .take_duration(Duration::from_millis(80))
                    .amplify(0.08);
                sink.append(tone1);
                sink.append(tone2);
                sink.sleep_until_end();
            }
        }
    });
}

/// Play a short falling tone to indicate recording stopped.
pub fn play_stop_sound() {
    std::thread::spawn(|| {
        if let Ok((_stream, handle)) = OutputStream::try_default() {
            if let Ok(sink) = Sink::try_new(&handle) {
                // Falling two-tone beep: 800Hz then 500Hz
                let tone1 = rodio::source::SineWave::new(800.0)
                    .take_duration(Duration::from_millis(80))
                    .amplify(0.08);
                let tone2 = rodio::source::SineWave::new(500.0)
                    .take_duration(Duration::from_millis(80))
                    .amplify(0.08);
                sink.append(tone1);
                sink.append(tone2);
                sink.sleep_until_end();
            }
        }
    });
}
