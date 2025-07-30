mod audio;
mod p2p;
mod tui;
mod video;

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use libp2p::{
    futures::StreamExt,
    gossipsub::{self, IdentTopic as Topic},
    multiaddr::Protocol,
    swarm::SwarmEvent,
    Multiaddr,
};
use std::error::Error;
use std::thread;
use tokio::{sync::mpsc, time::Duration};

use p2p::{
    AppBehaviourEvent, AudioData, ChatMessage, FileMessage, FrameData, AUDIO_TOPIC, CHAT_TOPIC,
    FILE_TOPIC, VIDEO_TOPIC,
};
use tui::Tui;

use p2p::AppStatus;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
enum Args {
    /// Create a new room and wait for others to join.
    Create,
    /// Join an existing room using a peer's address.
    Join {
        /// The address of the peer to connect to.
        #[arg(long)]
        address: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let mut tui = Tui::new()?;
    let mut camera = match video::initialize_camera() {
        Ok(camera) => Some(camera),
        Err(_) => None,
    };

    let (p2p_audio_sender, mut app_audio_receiver) = mpsc::unbounded_channel::<Vec<f32>>();
    let (app_audio_sender, p2p_audio_receiver) = mpsc::unbounded_channel::<Vec<f32>>();

    let (mut swarm, mut app_status) = match args {
        Args::Create => {
            let mut swarm = p2p::create_swarm(true).await?;
            let listen_addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse()?;
            swarm.listen_on(listen_addr)?;
            (swarm, AppStatus::WaitingForPeers)
        }
        Args::Join { address } => {
            let mut swarm = p2p::create_swarm(true).await?;
            let remote_addr: Multiaddr = address.parse()?;
            swarm.dial(remote_addr)?;
            (swarm, AppStatus::Joining)
        }
    };

    let _audio_streams = audio::setup_audio_streams(p2p_audio_sender, p2p_audio_receiver)?;

    let video_topic = Topic::new(VIDEO_TOPIC);
    let audio_topic = Topic::new(AUDIO_TOPIC);
    let chat_topic = Topic::new(CHAT_TOPIC);
    let file_topic = Topic::new(FILE_TOPIC);
    let local_peer_id = *swarm.local_peer_id();
    let local_peer_id_str = local_peer_id.to_string();

    let mut tick_interval = tokio::time::interval(Duration::from_millis(50));
    let (key_sender, mut key_receiver) = mpsc::unbounded_channel();
    let mut tui_dirty = true;
    let mut is_audio_muted = false;
    let mut is_video_muted = false;

    thread::spawn(move || {
        loop {
            match event::read() {
                Ok(event) => {
                    if key_sender.send(event).is_err() {
                        // rx closed
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    loop {
        if tui_dirty {
            match app_status {
                AppStatus::WaitingForPeers => {
                    tui.draw_waiting_for_peers(&local_peer_id_str)?;
                }
                AppStatus::Joining => {
                    tui.draw_joining()?;
                }
                AppStatus::InCall => {
                    // InCall is handled by the tick interval
                }
            }
            tui_dirty = false;
        }

        tokio::select! {
            _ = tick_interval.tick() => {
                if app_status == AppStatus::InCall {
                    // Process camera frame
                    let frame = if !is_video_muted {
                        if let Some(ref mut cam) = camera {
                            video::capture_and_process_frame(cam)
                                .unwrap_or_else(|_| video::create_no_camera_frame().unwrap())
                        } else {
                            video::create_no_camera_frame().unwrap()
                        }
                    } else {
                        video::create_no_camera_frame().unwrap()
                    };

                    // Send frame data along with mute status
                    let frame_data = FrameData {
                        peer_id: local_peer_id_str.clone(),
                        frame: frame.clone(),
                        is_audio_muted,
                        is_video_muted,
                    };
                    if let Ok(json) = serde_json::to_string(&frame_data) {
                        if let Err(_e) = swarm
                            .behaviour_mut()
                            .gossipsub
                            .publish(video_topic.clone(), json.as_bytes())
                        {
                        }
                    }

                    // Process and send audio if not muted
                    if !is_audio_muted {
                        if let Ok(audio_data) = app_audio_receiver.try_recv() {
                            let audio_data_p2p = AudioData {
                                peer_id: local_peer_id_str.clone(),
                                data: audio_data,
                            };
                            if let Ok(json) = serde_json::to_string(&audio_data_p2p) {
                                if let Err(_e) = swarm
                                    .behaviour_mut()
                                    .gossipsub
                                    .publish(audio_topic.clone(), json.as_bytes())
                                {
                                }
                            }
                        }
                    }
                    tui.draw(&frame, is_audio_muted, is_video_muted)?;
                }
            },
            key_event = key_receiver.recv() => {
                if let Some(Event::Key(key)) = key_event {
                    if key.kind == KeyEventKind::Press {
                        if tui.input_mode {
                            match key.code {
                                KeyCode::Char(c) => {
                                    tui.input.push(c);
                                    tui_dirty = true;
                                }
                                KeyCode::Backspace => {
                                    tui.input.pop();
                                    tui_dirty = true;
                                }
                                KeyCode::Enter => {
                                    let message_text: String = tui.input.drain(..).collect();
                                    let message = ChatMessage {
                                        peer_id: local_peer_id_str.clone(),
                                        message: message_text.clone(),
                                    };
                                    if let Ok(json) = serde_json::to_string(&message) {
                                        if let Err(_e) = swarm
                                            .behaviour_mut()
                                            .gossipsub
                                            .publish(chat_topic.clone(), json.as_bytes())
                                        {
                                        }
                                    }
                                    tui.messages.push(format!("You: {}", message_text));
                                    tui.input_mode = false;
                                    tui_dirty = true;
                                }
                                KeyCode::Esc => {
                                    tui.input.clear();
                                    tui.input_mode = false;
                                    tui_dirty = true;
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Char('q') => {
                                    p2p::end_call(&mut swarm)?;
                                    break;
                                }
                                KeyCode::Char('i') => {
                                    tui.input_mode = true;
                                    tui_dirty = true;
                                }
                                KeyCode::Char('m') => {
                                    is_audio_muted = !is_audio_muted;
                                    tui_dirty = true;
                                }
                                KeyCode::Char('v') => {
                                    is_video_muted = !is_video_muted;
                                    tui_dirty = true;
                                }
                                KeyCode::Char('f') => {
                                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                                        if let Ok(content) = std::fs::read(&path) {
                                            let file_name = path
                                                .file_name()
                                                .unwrap_or_default()
                                                .to_string_lossy()
                                                .to_string();
                                            let message = FileMessage {
                                                peer_id: local_peer_id_str.clone(),
                                                file_name: file_name.clone(),
                                                content,
                                            };
                                            if let Ok(json) = serde_json::to_string(&message) {
                                                if swarm
                                                    .behaviour_mut()
                                                    .gossipsub
                                                    .publish(file_topic.clone(), json.as_bytes())
                                                    .is_ok()
                                                {
                                                    tui.messages.push(format!(
                                                        "You sent a file: {}",
                                                        file_name
                                                    ));
                                                    tui_dirty = true;
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                } else if key_event.is_none() {
                    break;
                }
            },
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::ConnectionEstablished { .. } => {
                        app_status = AppStatus::InCall;
                        tui_dirty = true;
                    }
                    SwarmEvent::Dialing { .. } => {
                        // Not used in this context
                    }
                    SwarmEvent::ConnectionClosed { .. } => {
                        // Attempt to notify other peers, but don't error out if it fails
                        // (e.g. if we are the last peer).
                        let _ = p2p::end_call(&mut swarm);
                        break;
                    }
                    SwarmEvent::IncomingConnectionError { .. } => {
                        // Handle error
                    }
                    SwarmEvent::Behaviour(AppBehaviourEvent::Gossipsub(
                        gossipsub::Event::Message { message, .. },
                    )) => {
                        let topic = message.topic.as_str();
                        if topic == VIDEO_TOPIC {
                            if let Ok(frame_data) = serde_json::from_slice::<FrameData>(&message.data)
                            {
                                if frame_data.peer_id != local_peer_id_str {
                                    tui.update_frame(frame_data);
                                    tui_dirty = true;
                                }
                            }
                        } else if topic == AUDIO_TOPIC {
                            if let Ok(audio_data) =
                                serde_json::from_slice::<AudioData>(&message.data)
                            {
                                if audio_data.peer_id != local_peer_id_str {
                                    let _ = app_audio_sender.send(audio_data.data);
                                }
                            }
                        } else if topic == CHAT_TOPIC {
                            if let Ok(chat_message) =
                                serde_json::from_slice::<ChatMessage>(&message.data)
                            {
                                if chat_message.peer_id != local_peer_id_str {
                                    let peer_id_short = &chat_message.peer_id
                                        [chat_message.peer_id.len() - 6..];
                                    tui.messages.push(format!(
                                        "{}: {}",
                                        peer_id_short, chat_message.message
                                    ));
                                    tui_dirty = true;
                                }
                            }
                        } else if topic == FILE_TOPIC {
                            if let Ok(file_message) =
                                serde_json::from_slice::<FileMessage>(&message.data)
                            {
                                if file_message.peer_id != local_peer_id_str {
                                    let downloads_path =
                                        dirs::download_dir().unwrap_or_else(|| ".".into());
                                    let file_path =
                                        downloads_path.join(&file_message.file_name);
                                    if std::fs::write(&file_path, &file_message.content).is_ok()
                                    {
                                        let peer_id_short = &file_message.peer_id
                                            [file_message.peer_id.len() - 6..];
                                        tui.messages.push(format!(
                                            "{} sent a file: {} (saved to {})",
                                            peer_id_short,
                                            file_message.file_name,
                                            file_path.to_string_lossy()
                                        ));
                                        tui_dirty = true;
                                    }
                                }
                            }
                        } else if topic == p2p::CONTROL_TOPIC {
                            if let Ok(control_msg) =
                                serde_json::from_slice::<p2p::ControlMessage>(&message.data)
                            {
                                if control_msg == p2p::ControlMessage::EndCall {
                                    break;
                                }
                            }
                        }
                    }
                    SwarmEvent::NewListenAddr { address, .. } => {
                        let listen_addr = address.with(Protocol::P2p(local_peer_id));
                        tui.add_listen_address(listen_addr);
                        tui_dirty = true;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
