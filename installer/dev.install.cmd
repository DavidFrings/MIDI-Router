cargo build --release
mkdir "C:\Program Files\David Frings\dev"
copy ".\target\release\midi-router.exe" "C:\Program Files\David Frings\dev"
copy ".\target\release\updater.exe" "C:\Program Files\David Frings\dev"
copy ".\configs\akai-apc40-mk2\akai-apc40-mk2.config.toml" "C:\Program Files\David Frings\dev\config.toml"
