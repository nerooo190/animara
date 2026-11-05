# Homebrew cask template. Publish in a tap (e.g. nerooo190/homebrew-tap) and fill in sha256 per release:
#   brew install --cask nerooo190/tap/animara
cask "animara" do
  version "0.1.0"
  sha256 :no_check # replace with the SHA-256 of the universal DMG from SHA256SUMS.txt

  url "https://github.com/nerooo190/animara/releases/download/v#{version}/Animara-#{version}-macos-universal.dmg"
  name "Animara"
  desc "2D vector animation studio"
  homepage "https://github.com/nerooo190/animara"

  app "Animara.app"
  binary "#{appdir}/Animara.app/Contents/MacOS/animara-cli"
end
