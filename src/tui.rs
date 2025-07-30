use crate::p2p::FrameData;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use libp2p::Multiaddr;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::{
    collections::HashMap,
    io::{self, Stdout},
};

type Terminal = ratatui::Terminal<CrosstermBackend<Stdout>>;

fn draw_ui(frame: &mut Frame, content: impl Widget) {
    let size = frame.size();
    frame.render_widget(content, size);
}

pub struct Tui {
    terminal: Terminal,
    remote_frames: HashMap<String, String>,
    listen_addresses: Vec<Multiaddr>,
    pub messages: Vec<String>,
    pub input: String,
    pub input_mode: bool,
}

impl Tui {
    pub fn new() -> io::Result<Self> {
        let mut stdout = io::stdout();
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = ratatui::Terminal::new(backend)?;
        Ok(Self {
            terminal,
            remote_frames: HashMap::new(),
            listen_addresses: Vec::new(),
            messages: Vec::new(),
            input: String::new(),
            input_mode: false,
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
        let Tui {
            terminal,
            remote_frames,
            messages,
            input,
            input_mode,
            ..
        } = self;
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
                .split(f.size());

            let video_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(chunks[0]);

            let self_view = Paragraph::new(self_frame).block(
                Block::default()
                    .title("My View (Press 'q' to quit, 'i' to chat)")
                    .borders(Borders::ALL),
            );
            f.render_widget(self_view, video_chunks[0]);

            if !remote_frames.is_empty() {
                let remote_frame_text = remote_frames.values().next().unwrap().clone();
                let remote_peer_id = remote_frames.keys().next().unwrap().clone();
                let remote_view = Paragraph::new(remote_frame_text).block(
                    Block::default()
                        .title(format!("Peer: {}", remote_peer_id))
                        .borders(Borders::ALL),
                );
                f.render_widget(remote_view, video_chunks[1]);
            } else {
                let remote_view = Paragraph::new("Waiting for remote frame...")
                    .block(Block::default().title("Remote View").borders(Borders::ALL));
                f.render_widget(remote_view, video_chunks[1]);
            }

            let chat_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
                .split(chunks[1]);

            let message_items: Vec<ListItem> =
                messages.iter().map(|m| ListItem::new(m.as_str())).collect();
            let message_list = List::new(message_items)
                .block(Block::default().borders(Borders::ALL).title("Chat"));
            f.render_widget(message_list, chat_chunks[0]);

            let input_paragraph = Paragraph::new(input.as_str()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Input (Enter to send, Esc to exit)"),
            );
            f.render_widget(input_paragraph, chat_chunks[1]);

            if *input_mode {
                f.set_cursor(
                    chat_chunks[1].x + input.len() as u16 + 1,
                    chat_chunks[1].y + 1,
                );
            }
        })?;
        Ok(())
    }

    pub fn draw_waiting_for_peers(&mut self, local_peer_id: &str) -> io::Result<()> {
        let Tui {
            terminal,
            listen_addresses,
            ..
        } = self;
        let listen_addresses_str = listen_addresses
            .iter()
            .map(|addr| format!("  {}", addr))
            .collect::<Vec<_>>()
            .join("\n");
        terminal.draw(|f| {
            let text = format!(
                "Waiting for peers to join...\n\nYour Peer ID: {}\n\nListening on:\n{}",
                local_peer_id, listen_addresses_str
            );
            let paragraph =
                Paragraph::new(text).block(Block::default().title("Status").borders(Borders::ALL));
            draw_ui(f, paragraph);
        })?;
        Ok(())
    }

    pub fn draw_joining(&mut self) -> io::Result<()> {
        let Tui { terminal, .. } = self;
        terminal.draw(|f| {
            let paragraph = Paragraph::new("Joining room...")
                .block(Block::default().title("Status").borders(Borders::ALL));
            draw_ui(f, paragraph);
        })?;
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .unwrap();
        self.terminal.show_cursor().unwrap();
    }
}
