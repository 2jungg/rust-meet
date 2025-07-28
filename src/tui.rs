use crate::p2p::FrameData;
use crate::video;
use crossterm::{
    cursor, execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use libp2p::Multiaddr;
use std::collections::HashMap;
use std::io::{self, Stdout};

pub struct Tui {
    stdout: Stdout,
    remote_frames: HashMap<String, String>,
    listen_addresses: Vec<Multiaddr>,
}

impl Tui {
    pub fn new() -> io::Result<Self> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
        Ok(Self {
            stdout,
            remote_frames: HashMap::new(),
            listen_addresses: Vec::new(),
        })
    }

    pub fn add_listen_address(&mut self, addr: Multiaddr) {
        self.listen_addresses.push(addr);
    }

    pub fn update_frame(&mut self, frame_data: FrameData) {
        self.remote_frames
            .insert(frame_data.peer_id, frame_data.frame);
    }

    pub fn draw(&mut self, self_frame: &str) -> io::Result<()> {
        execute!(self.stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;

        // Draw self frame
        execute!(self.stdout, cursor::MoveTo(0, 0))?;
        println!("My View (Press 'q' to quit):\r");
        println!("{}\r", self_frame);

        // Draw remote frames
        let mut y_offset = 2 + video::OUTPUT_HEIGHT as u16;
        for (peer_id, frame) in &self.remote_frames {
            execute!(self.stdout, cursor::MoveTo(0, y_offset))?;
            println!("Peer: {}\r", peer_id);
            println!("{}\r", frame);
            y_offset += 2 + video::OUTPUT_HEIGHT as u16;
        }

        Ok(())
    }

    pub fn draw_waiting_for_peers(&mut self, local_peer_id: &str) -> io::Result<()> {
        execute!(self.stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        println!("Waiting for peers to join...\r");
        println!("Your Peer ID: {}\r", local_peer_id);
        println!("Listening on:\r");
        for addr in &self.listen_addresses {
            println!("  {}\r", addr);
        }
        Ok(())
    }

    pub fn draw_joining(&mut self) -> io::Result<()> {
        execute!(self.stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        println!("Joining room...\r");
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        execute!(self.stdout, LeaveAlternateScreen, cursor::Show).unwrap();
        terminal::disable_raw_mode().unwrap();
    }
}
