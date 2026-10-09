//! 彩蛋配乐：程序合成八比特风格的《Happy Birthday to You》，循环播放。
//! 旋律已进入公有领域。

use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamBuilder, Sink, Source};

const SAMPLE_RATE: u32 = 44_100;
/// 每分钟拍数。
const TEMPO: f32 = 132.0;
const VOLUME: f32 = 0.12;

/// 旋律：(MIDI 音高, 拍数)，音高为 `None` 表示休止。3/4 拍，带弱起。
const MELODY: [(Option<u8>, f32); 26] = [
    // Happy birthday to you
    (Some(67), 0.75),
    (Some(67), 0.25),
    (Some(69), 1.0),
    (Some(67), 1.0),
    (Some(72), 1.0),
    (Some(71), 2.0),
    // Happy birthday to you
    (Some(67), 0.75),
    (Some(67), 0.25),
    (Some(69), 1.0),
    (Some(67), 1.0),
    (Some(74), 1.0),
    (Some(72), 2.0),
    // Happy birthday dear ...
    (Some(67), 0.75),
    (Some(67), 0.25),
    (Some(79), 1.0),
    (Some(76), 1.0),
    (Some(72), 1.0),
    (Some(71), 1.0),
    (Some(69), 2.0),
    // Happy birthday to you
    (Some(77), 0.75),
    (Some(77), 0.25),
    (Some(76), 1.0),
    (Some(72), 1.0),
    (Some(74), 1.0),
    (Some(72), 2.0),
    // 下一遍之前停一拍
    (None, 1.0),
];

/// 正在播放的音乐。丢弃时停止。
pub struct Music {
    _sink: Sink,
    _stream: OutputStream,
}

/// 开始循环播放生日歌。没有可用的音频设备时返回 `None`，静默跳过。
pub fn play_birthday() -> Option<Music> {
    let mut stream = OutputStreamBuilder::open_default_stream().ok()?;
    // rodio 默认在丢弃时往 stderr 打印一行，会弄乱全屏界面
    stream.log_on_drop(false);
    let sink = Sink::connect_new(stream.mixer());
    let song = SamplesBuffer::new(1, SAMPLE_RATE, melody_samples(SAMPLE_RATE));
    sink.append(song.repeat_infinite());
    Some(Music {
        _sink: sink,
        _stream: stream,
    })
}

/// 把整首旋律合成为单声道采样。
fn melody_samples(rate: u32) -> Vec<f32> {
    let beat = 60.0 / TEMPO;
    let mut samples = Vec::new();
    for &(pitch, beats) in &MELODY {
        let len = (beats * beat * rate as f32) as usize;
        match pitch {
            Some(note) => samples.extend(tone(frequency(note), len, rate)),
            None => samples.extend(std::iter::repeat_n(0.0, len)),
        }
    }
    samples
}

fn frequency(midi: u8) -> f32 {
    440.0 * 2f32.powf((f32::from(midi) - 69.0) / 12.0)
}

/// 一个方波音符，带简单的包络：起音短促、逐渐衰减，结尾留一小段空白，
/// 这样连续的同音也能听出是两下。
fn tone(freq: f32, len: usize, rate: u32) -> impl Iterator<Item = f32> {
    let attack = (0.005 * rate as f32) as usize;
    let gap = (0.03 * rate as f32) as usize;
    let sounding = len.saturating_sub(gap).max(1);
    (0..len).map(move |i| {
        if i >= sounding {
            return 0.0;
        }
        let phase = (i as f32 * freq / rate as f32).fract();
        let square = if phase < 0.5 { 1.0 } else { -1.0 };
        let rise = (i as f32 / attack.max(1) as f32).min(1.0);
        let decay = 1.0 - 0.6 * (i as f32 / sounding as f32);
        square * rise * decay * VOLUME
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn melody_has_expected_length_and_volume() {
        let samples = melody_samples(SAMPLE_RATE);
        let beats: f32 = MELODY.iter().map(|&(_, b)| b).sum();
        let seconds = samples.len() as f32 / SAMPLE_RATE as f32;
        assert!((seconds - beats * 60.0 / TEMPO).abs() < 0.05, "{seconds}");
        assert!(samples.iter().all(|s| s.abs() <= VOLUME));
        assert!(samples.iter().any(|s| s.abs() > VOLUME / 2.0));
    }

    #[test]
    fn pitches_match_concert_tuning() {
        assert!((frequency(69) - 440.0).abs() < 1e-3);
        assert!((frequency(72) - 523.25).abs() < 0.01);
    }
}
