use libp2p::{
    gossipsub::{self, IdentTopic as Topic, MessageAuthenticity},
    identity, mdns, noise,
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour},
    tcp, yamux, PeerId, Swarm, SwarmBuilder,
};
use serde::{Deserialize, Serialize};
use std::error::Error;

pub const VIDEO_TOPIC: &str = "video";
pub const AUDIO_TOPIC: &str = "audio";
pub const CHAT_TOPIC: &str = "chat";
pub const CONTROL_TOPIC: &str = "control";
pub const FILE_TOPIC: &str = "file";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ControlMessage {
    EndCall,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum AppStatus {
    WaitingForPeers,
    Joining,
    InCall,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FrameData {
    pub peer_id: String,
    pub frame: String, // ASCII frame
    pub is_audio_muted: bool,
    pub is_video_muted: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AudioData {
    pub peer_id: String,
    pub data: Vec<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub peer_id: String,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMessage {
    pub peer_id: String,
    pub file_name: String,
    pub content: Vec<u8>,
}

// The network behaviour combines multiple protocols.
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "AppBehaviourEvent")]
pub struct AppBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: Toggle<mdns::tokio::Behaviour>,
}

#[derive(Debug)]
pub enum AppBehaviourEvent {
    Gossipsub(gossipsub::Event),
    Mdns(()),
}

impl From<gossipsub::Event> for AppBehaviourEvent {
    fn from(event: gossipsub::Event) -> Self {
        AppBehaviourEvent::Gossipsub(event)
    }
}

impl From<mdns::Event> for AppBehaviourEvent {
    fn from(_: mdns::Event) -> Self {
        AppBehaviourEvent::Mdns(())
    }
}

pub async fn create_swarm(use_mdns: bool) -> Result<Swarm<AppBehaviour>, Box<dyn Error>> {
    // Create a random PeerId
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Local peer id: {}", local_peer_id);

    // Create a Gossipsub topic
    let video_topic = Topic::new(VIDEO_TOPIC);
    let audio_topic = Topic::new(AUDIO_TOPIC);
    let chat_topic = Topic::new(CHAT_TOPIC);
    let control_topic = Topic::new(CONTROL_TOPIC);
    let file_topic = Topic::new(FILE_TOPIC);

    // Create a Swarm to manage peers and events
    let swarm = {
        let gossipsub_config = gossipsub::Config::default();
        let mut gossipsub: gossipsub::Behaviour = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )
        .map_err(|msg| std::io::Error::new(std::io::ErrorKind::Other, msg))?;
        gossipsub.subscribe(&video_topic)?;
        gossipsub.subscribe(&audio_topic)?;
        gossipsub.subscribe(&chat_topic)?;
        gossipsub.subscribe(&control_topic)?;
        gossipsub.subscribe(&file_topic)?;

        let mdns = if use_mdns {
            Some(mdns::tokio::Behaviour::new(
                mdns::Config::default(),
                local_peer_id,
            )?)
            .into()
        } else {
            None.into()
        };

        let behaviour = AppBehaviour { gossipsub, mdns };

        SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|_key| behaviour)?
            .with_swarm_config(|c| {
                c.with_idle_connection_timeout(std::time::Duration::from_secs(60))
            })
            .build()
    };

    Ok(swarm)
}

pub fn end_call(swarm: &mut Swarm<AppBehaviour>) -> Result<(), Box<dyn Error>> {
    let control_topic = Topic::new(CONTROL_TOPIC);
    let message = serde_json::to_string(&ControlMessage::EndCall)?;
    swarm
        .behaviour_mut()
        .gossipsub
        .publish(control_topic, message.as_bytes())?;
    Ok(())
}
