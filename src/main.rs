mod p2p;
mod video;
mod audio;
mod tui;

use clap::Parser;
use libp2p::{futures::StreamExt, gossipsub::{self, IdentTopic as Topic}, swarm::SwarmEvent, Multiaddr, mdns};
use std::error::Error;
use tokio::sync::mpsc;

use p2p::{FrameData, AudioData, VIDEO_TOPIC, AUDIO_TOPIC, AppBehaviourEvent};
use tui::Tui;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The address to listen on for incoming connections.
    #[arg(short, long)]
    listen_address: Option<String>,

    /// The address of a peer to connect to.
    #[arg(short, long)]
    peer_address: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut tui = Tui::new()?;
    let mut camera = video::initialize_camera()?;

    let (p2p_audio_sender, mut app_audio_receiver) = mpsc::unbounded_channel::<Vec<f32>>();
    let (app_audio_sender, p2p_audio_receiver) = mpsc::unbounded_channel::<Vec<f32>>();

    let mut swarm = p2p::create_swarm().await?;

    if let Some(listen_addr) = args.listen_address {
        let addr: Multiaddr = listen_addr.parse()?;
        swarm.listen_on(addr)?;
        println!("Listening on {}", listen_addr);
    }

    if let Some(peer_addr) = args.peer_address {
        let addr: Multiaddr = peer_addr.parse()?;
        swarm.dial(addr)?;
        println!("Dialed {}", peer_addr);
    }

    let _audio_streams = audio::setup_audio_streams(p2p_audio_sender, p2p_audio_receiver)?;

    let video_topic = Topic::new(VIDEO_TOPIC);
    let audio_topic = Topic::new(AUDIO_TOPIC);
    let local_peer_id = swarm.local_peer_id().to_string();

    loop {
        tui.handle_events()?;
        if tui.should_quit() {
            break;
        }

        // Process camera frame
        if let Ok(frame) = video::capture_and_process_frame(&mut camera) {
            let frame_data = FrameData {
                peer_id: local_peer_id.clone(),
                frame: frame.clone(),
            };
            if let Ok(json) = serde_json::to_string(&frame_data) {
                swarm.behaviour_mut().gossipsub.publish(video_topic.clone(), json.as_bytes())?;
            }
            tui.draw(&frame)?;
        }

        // Process audio
        if let Ok(audio_data) = app_audio_receiver.try_recv() {
             let audio_data_p2p = AudioData {
                peer_id: local_peer_id.clone(),
                data: audio_data,
            };
            if let Ok(json) = serde_json::to_string(&audio_data_p2p) {
                swarm.behaviour_mut().gossipsub.publish(audio_topic.clone(), json.as_bytes())?;
            }
        }

        // Handle network events
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::Behaviour(AppBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                        for (peer_id, _multiaddr) in list {
                            println!("mDNS discovered a new peer: {}", peer_id);
                            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                        }
                    }
                    SwarmEvent::Behaviour(AppBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                        for (peer_id, _multiaddr) in list {
                            println!("mDNS discover peer has expired: {}", peer_id);
                            swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                        }
                    }
                    SwarmEvent::Behaviour(AppBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                        message,
                        ..
                    })) => {
                        let topic = message.topic.as_str();
                        if topic == VIDEO_TOPIC {
                            if let Ok(frame_data) = serde_json::from_slice::<FrameData>(&message.data) {
                                if frame_data.peer_id != local_peer_id {
                                    tui.update_frame(frame_data);
                                }
                            }
                        } else if topic == AUDIO_TOPIC {
                            if let Ok(audio_data) = serde_json::from_slice::<AudioData>(&message.data) {
                                if audio_data.peer_id != local_peer_id {
                                    let _ = app_audio_sender.send(audio_data.data);
                                }
                            }
                        }
                    }
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Listening on {}", address);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
