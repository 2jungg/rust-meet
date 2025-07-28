mod audio;
mod p2p;
mod tui;
mod video;

use clap::Parser;
use libp2p::{
    futures::StreamExt,
    gossipsub::{self, IdentTopic as Topic},
    multiaddr::Protocol,
    swarm::SwarmEvent,
    Multiaddr,
};
use std::error::Error;
use tokio::sync::mpsc;

use p2p::{AppBehaviourEvent, AudioData, FrameData, AUDIO_TOPIC, VIDEO_TOPIC};
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
    let local_peer_id = *swarm.local_peer_id();
    let local_peer_id_str = local_peer_id.to_string();

    loop {
        if tui.should_quit() {
            p2p::end_call(&mut swarm)?;
            break;
        }
        tui.handle_events(&mut app_status)?;

        match app_status {
            AppStatus::WaitingForPeers => {
                tui.draw_waiting_for_peers()?;
            }
            AppStatus::Joining => {
                tui.draw_joining()?;
            }
            AppStatus::InCall => {
                // Process camera frame
                let frame = if let Some(ref mut cam) = camera {
                    video::capture_and_process_frame(cam)
                        .unwrap_or_else(|_| video::create_no_camera_frame().unwrap())
                } else {
                    video::create_no_camera_frame().unwrap()
                };

                let frame_data = FrameData {
                    peer_id: local_peer_id_str.clone(),
                    frame: frame.clone(),
                };
                if let Ok(json) = serde_json::to_string(&frame_data) {
                    swarm
                        .behaviour_mut()
                        .gossipsub
                        .publish(video_topic.clone(), json.as_bytes())?;
                }
                tui.draw(&frame)?;

                // Process audio
                if let Ok(audio_data) = app_audio_receiver.try_recv() {
                    let audio_data_p2p = AudioData {
                        peer_id: local_peer_id_str.clone(),
                        data: audio_data,
                    };
                    if let Ok(json) = serde_json::to_string(&audio_data_p2p) {
                        swarm
                            .behaviour_mut()
                            .gossipsub
                            .publish(audio_topic.clone(), json.as_bytes())?;
                    }
                }
            }
        }

        // Handle network events
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::ConnectionEstablished { .. } => {
                        app_status = AppStatus::InCall;
                    }
                    SwarmEvent::Dialing { .. } => {
                        // Not used in this context
                    }
                    SwarmEvent::ConnectionClosed { .. } => {
                        // Handle disconnection if necessary
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
                        println!("Listening on: {}", address.with(Protocol::P2p(local_peer_id)));
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
