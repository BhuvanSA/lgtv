# Rust LG TV Controller with Karabiner

A blazing fast Rust application that allows you to control your LG TV volume using your Mac's keyword volume keys. It seamlessly integrates with Karabiner Elements and only sends commands to the TV when it is the active audio output device.

## Features
-   **Smart Control**: Controls TV volume ONLY when "LG Monitor" is your active audio output.
-   **Native Fallback**: Seamlessly allows macOS to handle volume for invalid devices (Speakers/Headphones) via Karabiner passthrough.
-   **Ultra-Low Latency**: Uses a native Swift helper for sub-10ms audio device detection.
-   **Resilient Connectivity**: Auto-reconnects in the background if the TV turns off or disconnects.
-   **No Lag**: Uses a Named Pipe (FIFO) for instant command delivery from Karabiner.
-   **Background Service**: Runs silently in the background on startup.

> **Security Note**: This application requires your TV's MAC address to be set in an environment variable (`LGTV_MAC`) to avoid hardcoding sensitive network information in the source code.

## Prerequisites
-   **Rust**: [Install Rust](https://www.rust-lang.org/tools/install)
-   **Karabiner-Elements**: [Install Karabiner](https://karabiner-elements.pqrs.org/)

## Setup Guide

### 1. Build and Install
Compile the application and the audio helper:
```bash
# Build Rust App
cargo build --release
# Build Swift Audio Helper
swiftc src/audio.swift -o get_audio_device
```

Create a local bin directory (if you don't have one) and copy the binaries:
```bash
mkdir -p ~/.local/bin
cp target/release/lgtv ~/.local/bin/
cp get_audio_device ~/.local/bin/
```

### 2. Configure Karabiner
1.  Open Karabiner-Elements.
2.  Go to `Complex Modifications` -> `Add rule`.
3.  Currently, we provided a `karabiner.json` file in this repository. 
    You can copy the content of `karabiner.json` into `~/.config/karabiner/karabiner.json` (inside the `"rules": []` array) or add it via the UI.

    *Note: The rule is robust and handles both Media keys (Volume Up/Down) and F-keys (F10/F11/F12).*

### 3. Setup Startup Service
To have the controller run automatically when you log in:

1.  **Configure the MAC Address**: Open `com.user.lgtv.plist` and replace the `LGTV_MAC` value with your TV's actual MAC address (e.g., `3C:F0:83:9E:6A:2C`).
2.  Copy the plist file to your LaunchAgents folder:
    ```bash
    cp com.user.lgtv.plist ~/Library/LaunchAgents/
    ```
3.  Load the service:
    ```bash
    launchctl load ~/Library/LaunchAgents/com.user.lgtv.plist
    ```

### 4. Initial Pairing
1.  Run the app manually once to pair with your TV:
    ```bash
    ~/.local/bin/lgtv
    ```
2.  **Accept the prompt on your LG TV.**
3.  Once connected, press `Ctrl+C` to stop it. The pairing key is saved to `~/.lgtv_key`.

## Usage
Simply press your volume keys! 
-   If **LG Monitor** is selected as your sound output, the TV volume changes.
-   If any other device is selected, your Mac handles the volume natively (HUD appears).

## Troubleshooting
-   **Logs**: Check the service logs at `/tmp/lgtv.log` and `/tmp/lgtv.err`. 
-   **Reconnection**: If you turned off the TV, the service might take up to 5 seconds to detect it's back on the network and reconnect.
