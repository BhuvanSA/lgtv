#![allow(unused_imports)]
use std::path::Path;
use std::time::Duration;
use std::process::Command;
use nix::sys::stat::Mode;
use nix::unistd::mkfifo;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio::time::sleep;
use lg_webos_client::client::{WebosClient, WebOsClientConfig};
use lg_webos_client::command::Command as WebOsCommand;
use home::home_dir;
use regex::Regex;
use tokio_tungstenite::{WebSocketStream, MaybeTlsStream};
use tokio_tungstenite::tungstenite::Message;
use tokio::net::TcpStream;
use futures_util::stream::SplitSink;

/// LG TV Controller with Passthrough
/// 
/// Architecture:
/// 1. Fast Path: Uses a native Swift helper (`get_audio_device`) to check the active output in <10ms.
/// 2. Passthrough: Karabiner sends commands EVERY keypress. This app ignores them if the TV
///    isn't the active audio device, allowing macOS to handle the volume natively.
/// 3. Resilient: Automatically reconnects if the TV is turned off or the network drops.

const PIPE_PATH: &str = "/tmp/lgtv-pipe";

// The name reported by CoreAudio for the LG TV when used as an audio sink.
const TARGET_AUDIO_DEVICE: &str = "LG Monitor";

// The MAC address of your LG TV.
const TARGET_MAC: &str = "3C:F0:83:9E:6A:2C";

type ClientType = WebosClient<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>;

fn map_client_error<E: std::fmt::Debug>(e: E) -> Box<dyn std::error::Error> {
    format!("{:?}", e).into()
}

/// Resolves the IP address of the LG TV using its MAC address.
fn resolve_ip_from_mac() -> Option<String> {
    let output = Command::new("arp").arg("-an").output().ok()?;
    let output_str = String::from_utf8_lossy(&output.stdout);
    let normalized_target = TARGET_MAC.to_lowercase();
    let re = Regex::new(r"\(([^)]+)\) at ([0-9a-f:]+)").ok()?;

    for line in output_str.lines() {
        if let Some(caps) = re.captures(line) {
            let ip = caps.get(1)?.as_str().to_string();
            let mac = caps.get(2)?.as_str().to_string();
            if mac.to_lowercase() == normalized_target {
                return Some(ip);
            }
        }
    }
    None
}

/// Check if the target LG device is currently active in macOS.
fn is_lg_tv_active_audio() -> bool {
    let home = home_dir().expect("Cannot find home directory");
    let swift_bin = home.join(".local/bin/get_audio_device");
    let local_bin = std::env::current_dir().unwrap_or_default().join("get_audio_device");

    let cmd_path = if swift_bin.exists() {
        swift_bin
    } else if local_bin.exists() {
        local_bin
    } else {
        return false;
    };

    if let Ok(out) = Command::new(cmd_path).output() {
        let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return name.eq_ignore_ascii_case(TARGET_AUDIO_DEVICE);
    }
    false
}

enum AppEvent {
    CommandReceived(String),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting LG TV Controller...");

    use std::os::unix::fs::PermissionsExt;

    // 1. Setup Named Pipe
    if !Path::new(PIPE_PATH).exists() {
        mkfifo(PIPE_PATH, Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IRWXO).ok();
        if let Ok(metadata) = std::fs::metadata(PIPE_PATH) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o666);
            let _ = std::fs::set_permissions(PIPE_PATH, perms);
        }
    }

    // 2. Spawn Command Listener
    let (tx, mut rx) = mpsc::channel(32);
    let tx_clone = tx.clone();
    
    tokio::spawn(async move {
        loop {
            // Open RDWR to prevent EOF loops when no writers are present.
            match OpenOptions::new().read(true).write(true).open(PIPE_PATH).await {
                Ok(file) => {
                    let mut reader = BufReader::new(file);
                    let mut line = String::new();
                    while let Ok(n) = reader.read_line(&mut line).await {
                        if n == 0 { break; } 
                        let cmd = line.trim().to_string();
                        if !cmd.is_empty() {
                             let _ = tx_clone.send(AppEvent::CommandReceived(cmd)).await;
                        }
                        line.clear();
                    }
                }
                Err(_) => { sleep(Duration::from_secs(1)).await; }
            }
        }
    });

    // 3. Main Loop: Handles connection state and TV commands.
    let mut client: Option<ClientType> = None;
    
    loop {
        // A) Connection/Re-connection handling
        if client.is_none() {
             if let Some(ip) = resolve_ip_from_mac() {
                let key_path = home_dir().expect("Cannot find home").join(".lgtv_key");
                let key = if key_path.exists() {
                    tokio::fs::read_to_string(&key_path).await.ok().map(|k| k.trim().to_string())
                } else { None };

                let url = format!("ws://{}:3000/", ip);
                let config = WebOsClientConfig::new(&url, key.clone());
                
                if let Ok(c) = WebosClient::new(config).await {
                    println!("Connected to TV at {}", ip);
                    
                    // Persist the pairing key if it's new or changed.
                    if let Some(new_key) = &c.key {
                        let new_key_trimmed = new_key.trim();
                        if key.as_deref() != Some(new_key_trimmed) {
                            let _ = tokio::fs::write(&key_path, new_key_trimmed.as_bytes()).await;
                        }
                    }
                    client = Some(c);
                }
             }
        }

        // B) Process Incoming Commands
        let wait_time = if client.is_none() { Duration::from_secs(5) } else { Duration::from_secs(3600) };
        
        match tokio::time::timeout(wait_time, rx.recv()).await {
            Ok(Some(AppEvent::CommandReceived(cmd))) => {
                // Ignore commands if the TV isn't the active audio device.
                if !is_lg_tv_active_audio() && (cmd.starts_with("volume") || cmd == "mute" || cmd == "unmute") {
                     continue;
                }

                if let Some(c) = &client {
                    let result = async {
                        match cmd.as_str() {
                            "volume_up" => {
                                let resp = c.send_command(WebOsCommand::GetVolume).await.map_err(map_client_error)?;
                                if let Some(p) = resp.payload {
                                   if let Some(vol) = p.get("volumeStatus").and_then(|s| s.get("volume")).and_then(|v| v.as_i64()) {
                                       let nv = (vol + 1).max(0).min(100);
                                       let _ = c.send_command(WebOsCommand::SetVolume(nv as i8)).await;
                                   }
                                }
                                Ok::<(), Box<dyn std::error::Error>>(()) 
                            },
                             "volume_down" => {
                                let resp = c.send_command(WebOsCommand::GetVolume).await.map_err(map_client_error)?;
                                if let Some(p) = resp.payload {
                                   if let Some(vol) = p.get("volumeStatus").and_then(|s| s.get("volume")).and_then(|v| v.as_i64()) {
                                       let nv = (vol - 1).max(0).min(100);
                                       let _ = c.send_command(WebOsCommand::SetVolume(nv as i8)).await;
                                   }
                                }
                                Ok(()) 
                            },
                            "mute" => { let _ = c.send_command(WebOsCommand::SetMute(true)).await; Ok(()) },
                            "unmute" => { let _ = c.send_command(WebOsCommand::SetMute(false)).await; Ok(()) },
                            _ => Ok(())
                        }
                    }.await;

                    if result.is_err() {
                        client = None; 
                    }
                }
            },
            Ok(None) => break, 
            Err(_) => { }
        }
    }
    Ok(())
}
