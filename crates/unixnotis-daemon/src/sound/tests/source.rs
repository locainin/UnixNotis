use std::fs;

use super::SoundFile;
use crate::test_support::TempRoot;

#[test]
fn playback_path_uses_the_retained_descriptor() {
    let root = TempRoot::new("sound-source");
    let path = root.join("alert.wav");
    fs::write(&path, b"descriptor-backed sound").expect("write sound fixture");
    let file = fs::File::open(&path).expect("open sound fixture");
    let sound = SoundFile::new(path, file);

    let playback_path = sound.playback_path();

    assert!(playback_path.starts_with(format!("/proc/{}/fd", std::process::id())));
    assert_eq!(
        fs::read(playback_path).expect("read retained descriptor path"),
        b"descriptor-backed sound"
    );
}
