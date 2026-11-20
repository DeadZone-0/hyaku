use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use tempfile::TempDir;
use tokio::process::{Child, Command};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cue {
    MenuMove,
    MenuConfirm,
    Swing,
    Impact,
    Hurt,
    Ascend,
    Shrine,
    Raid,
    Fallen,
}

impl Cue {
    const fn file(self) -> &'static str {
        match self {
            Self::MenuMove => "menu-move.wav",
            Self::MenuConfirm => "menu-confirm.wav",
            Self::Swing => "swing.wav",
            Self::Impact => "impact.wav",
            Self::Hurt => "hurt.wav",
            Self::Ascend => "ascend.wav",
            Self::Shrine => "shrine.wav",
            Self::Raid => "raid.wav",
            Self::Fallen => "fallen.wav",
        }
    }

    fn is_milestone(self) -> bool {
        matches!(self, Self::Ascend | Self::Shrine | Self::Raid)
    }
}

const AMBIENCE: &[u8] = include_bytes!("../audio/ambience.pcm");
const SAMPLE_RATE: u32 = 24_000;
const FADE_SECONDS: f32 = 0.45;
const CLIPS: &[(&str, &[u8])] = &[
    (Cue::MenuMove.file(), include_bytes!("../audio/menu-move.wav")),
    (Cue::MenuConfirm.file(), include_bytes!("../audio/menu-confirm.wav")),
    (Cue::Swing.file(), include_bytes!("../audio/swing.wav")),
    (Cue::Impact.file(), include_bytes!("../audio/impact.wav")),
    (Cue::Hurt.file(), include_bytes!("../audio/hurt.wav")),
    (Cue::Ascend.file(), include_bytes!("../audio/ascend.wav")),
    (Cue::Shrine.file(), include_bytes!("../audio/shrine.wav")),
    (Cue::Raid.file(), include_bytes!("../audio/raid.wav")),
    (Cue::Fallen.file(), include_bytes!("../audio/fallen.wav")),
];

pub struct Sound {
    players: VecDeque<(PathBuf, &'static [&'static str])>,
    ambience: Option<Ambience>,
    effect: Option<(Cue, Child)>,
    milestones: VecDeque<Cue>,
    clips: TempDir,
}

impl Sound {
    pub fn open() -> Option<Self> {
        let path = std::env::var_os("PATH")?;
        let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
            &[("afplay", &[]), ("ffplay", &["-nodisp", "-autoexit", "-loglevel", "quiet"])]
        } else {
            &[
                ("pw-play", &[]),
                ("paplay", &[]),
                ("ffplay", &["-nodisp", "-autoexit", "-loglevel", "quiet"]),
                ("aplay", &["-q"]),
            ]
        };
        let players: VecDeque<_> = candidates.iter().filter_map(|&(name, args)| {
            let executable = std::env::split_paths(&path)
                .map(|dir| dir.join(name))
                .find(|file| file.is_file())?;
            Some((executable, args))
        }).collect();
        if players.is_empty() {
            return None;
        }

        let clips = tempfile::Builder::new().prefix("hyaku-audio-").tempdir().ok()?;
        for &(name, wav) in CLIPS {
            std::fs::write(clips.path().join(name), wav).ok()?;
        }
        #[cfg(target_os = "macos")]
        let ambience = Ambience::open();
        #[cfg(not(target_os = "macos"))]
        let ambience = None;
        Some(Self {
            players, ambience, effect: None, milestones: VecDeque::new(), clips,
        })
    }

    pub fn update(&mut self, ambience_level: f32, mut cue: Option<Cue>, milestones: &[Cue]) {
        self.milestones.extend(milestones.iter().copied());
        let mut backend_failed = false;
        #[cfg(not(target_os = "macos"))]
        if let Some(done) = self.ambience.as_mut().and_then(|ambience| finished(&mut ambience.child)) {
            self.ambience = None;
            backend_failed = !done;
        }
        if let Some((playing, child)) = &mut self.effect {
            if let Some(done) = finished(child) {
                if !done {
                    if playing.is_milestone() {
                        self.milestones.push_front(*playing);
                    } else {
                        cue = cue.max(Some(*playing));
                    }
                    backend_failed = true;
                }
                self.effect = None;
            }
        }
        if backend_failed {
            self.players.pop_front();
            #[cfg(not(target_os = "macos"))]
            {
                self.ambience = None;
            }
            self.effect = None;
        }

        #[cfg(not(target_os = "macos"))]
        if self.ambience.is_none() && ambience_level > 0.0 {
            while let Some((executable, _)) = self.players.front() {
                self.ambience = Ambience::open(executable);
                if self.ambience.is_some() {
                    break;
                }
                self.players.pop_front();
            }
        }

        if let Some(ambience) = &mut self.ambience {
            ambience.set_level(ambience_level);
        }

        if cue == Some(Cue::Fallen) {
            self.milestones.clear();
        } else if !self.effect.as_ref().is_some_and(|(playing, _)| {
            playing.is_milestone() || *playing == Cue::Fallen
        }) {
            cue = self.milestones.pop_front().or(cue);
        }

        if let Some(cue) = cue {
            if self.effect.as_ref().map_or(true, |(playing, _)| cue > *playing) {
                self.effect = None;
                self.effect = self.play(cue.file()).map(|child| (cue, child));
            }
        }
    }

    fn play(&mut self, file: &str) -> Option<Child> {
        while let Some((executable, args)) = self.players.front() {
            match Command::new(executable)
                .args(*args)
                .arg(self.clips.path().join(file))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()
            {
                Ok(child) => return Some(child),
                Err(_) => {
                    self.players.pop_front();
                }
            }
        }
        None
    }
}

fn finished(child: &mut Child) -> Option<bool> {
    match child.try_wait() {
        Ok(status) => status.map(|code| code.success()),
        Err(_) => Some(false),
    }
}

fn fade_toward(level: f32, target: f32, seconds: f32) -> f32 {
    let step = seconds / FADE_SECONDS;
    level + (target - level).clamp(-step, step)
}

#[cfg(not(target_os = "macos"))]
struct Ambience {
    child: Child,
    level: tokio::sync::watch::Sender<f32>,
}

#[cfg(not(target_os = "macos"))]
impl Ambience {
    fn open(executable: &std::path::Path) -> Option<Self> {
        let args: &[&str] = match executable.file_name()?.to_str()? {
            "pw-play" => &["--raw", "--rate=24000", "--channels=1", "--format=s16", "--latency=60ms", "-"],
            "paplay" => &["--raw", "--rate=24000", "--channels=1", "--format=s16le", "--latency-msec=60"],
            "ffplay" => &["-nodisp", "-autoexit", "-loglevel", "quiet", "-f", "s16le", "-ar", "24000", "-ch_layout", "mono", "-i", "pipe:0"],
            "aplay" => &["-q", "-t", "raw", "-f", "S16_LE", "-r", "24000", "-c", "1", "-F", "20000", "-B", "80000"],
            _ => return None,
        };
        let mut child = Command::new(executable)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .ok()?;
        let mut pipe = child.stdin.take()?;
        let (level, target) = tokio::sync::watch::channel(0.0);
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            use tokio::time::{interval, Duration, MissedTickBehavior};

            const FRAMES: usize = SAMPLE_RATE as usize / 50;
            let mut clock = interval(Duration::from_millis(20));
            clock.set_missed_tick_behavior(MissedTickBehavior::Delay);
            let mut block = [0; FRAMES * 2];
            let mut samples = AMBIENCE.chunks_exact(2).cycle();
            let mut gain = 0.0;
            while target.has_changed().is_ok() {
                clock.tick().await;
                let goal = *target.borrow();
                for dest in block.chunks_exact_mut(2) {
                    gain = fade_toward(gain, goal, 1.0 / SAMPLE_RATE as f32);
                    let sample = samples.next().unwrap();
                    let scaled = (f32::from(i16::from_le_bytes([sample[0], sample[1]])) * gain) as i16;
                    dest.copy_from_slice(&scaled.to_le_bytes());
                }
                if pipe.write_all(&block).await.is_err() {
                    break;
                }
            }
        });
        Some(Self { child, level })
    }

    fn set_level(&self, new_level: f32) {
        self.level.send_replace(new_level);
    }
}

#[cfg(target_os = "macos")]
struct Ambience {
    _stream: rodio::OutputStream,
    sink: rodio::Sink,
    level: f32,
    updated: std::time::Instant,
}

#[cfg(target_os = "macos")]
impl Ambience {
    fn open() -> Option<Self> {
        use rodio::Source;

        let mut stream = rodio::OutputStreamBuilder::open_default_stream().ok()?;
        stream.log_on_drop(false);
        let sink = rodio::Sink::connect_new(stream.mixer());
        sink.set_volume(0.0);
        let samples = AMBIENCE.chunks_exact(2)
            .map(|sample| f32::from(i16::from_le_bytes([sample[0], sample[1]])) / 32768.0)
            .collect::<Vec<_>>();
        sink.append(rodio::buffer::SamplesBuffer::new(1, SAMPLE_RATE, samples).repeat_infinite());
        Some(Self { _stream: stream, sink, level: 0.0, updated: std::time::Instant::now() })
    }

    fn set_level(&mut self, new_level: f32) {
        self.level = fade_toward(self.level, new_level, self.updated.elapsed().as_secs_f32());
        self.updated = std::time::Instant::now();
        self.sink.set_volume(self.level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambience_can_reverse_a_fade_without_a_jump() {
        let half = fade_toward(1.0, 0.0, FADE_SECONDS / 2.0);
        assert_eq!(half, 0.5);
        assert_eq!(fade_toward(half, 1.0, 0.0), half);
        assert_eq!(fade_toward(half, 1.0, FADE_SECONDS / 4.0), 0.75);
        assert_eq!(fade_toward(half, 0.0, FADE_SECONDS), 0.0);
    }
}
