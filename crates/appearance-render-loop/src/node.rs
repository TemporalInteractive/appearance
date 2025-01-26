use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use std::thread;
use uuid::Uuid;

use crate::host::{HostMessage, HostMessageType};

#[derive(bytemuck::NoUninit, bytemuck::AnyBitPattern, Clone, Copy, Default)]
#[repr(C)]
pub struct NodeScissor {
    pub scissor_x: [u32; 2],
    pub scissor_y: [u32; 2],
}

impl NodeScissor {
    pub fn width(&self) -> u32 {
        self.scissor_x[1] - self.scissor_x[0]
    }

    pub fn height(&self) -> u32 {
        self.scissor_y[1] - self.scissor_y[0]
    }
}

pub trait NodeRenderer: Send {
    // TODO: world manipulation

    fn render(&mut self, width: u32, height: u32, scissor: NodeScissor) -> Vec<u8>;
}

#[derive(bytemuck::NoUninit, Clone, Copy)]
#[repr(u32)]
pub enum NodeMessageType {
    Connect,
    Disconnect,
    RenderStarted,
    RenderFinished,
}

impl From<u32> for NodeMessageType {
    fn from(value: u32) -> Self {
        match value {
            0 => NodeMessageType::Connect,
            1 => NodeMessageType::Disconnect,
            2 => NodeMessageType::RenderStarted,
            3 => NodeMessageType::RenderFinished,
            _ => panic!(),
        }
    }
}

#[derive(bytemuck::NoUninit, bytemuck::AnyBitPattern, Clone, Copy)]
#[repr(C)]
pub struct NodeMessage {
    ty: u32,
    node_id: [u8; 16],
}

impl NodeMessage {
    fn new(ty: NodeMessageType, node_id: Uuid) -> Self {
        Self {
            ty: ty as u32,
            node_id: node_id.to_bytes_le(),
        }
    }

    pub fn ty(&self) -> NodeMessageType {
        self.ty.into()
    }

    pub fn node_id(&self) -> Uuid {
        Uuid::from_bytes(self.node_id)
    }
}

struct NodeState<T: NodeRenderer> {
    id: Uuid,
    host_addr: String,
    node_addr: String,
    tcp_client: Option<TcpStream>,
    renderer: T,
}

impl<T: NodeRenderer> NodeState<T> {
    fn tcp_client_write_all(&mut self, bytes: &[u8]) {
        if self.tcp_client.is_none() {
            if let Ok(tcp_client) = TcpStream::connect(&self.host_addr) {
                log::info!("Tcp connected");
                self.tcp_client = Some(tcp_client);
            }
        }

        if let Some(tcp_client) = &mut self.tcp_client {
            if tcp_client.write_all(bytes).is_err() {
                log::info!("Tcp disconnected");
                self.tcp_client = None;
            } else {
                log::info!("Tcp write all succes");
            }
        }
    }
}

pub struct Node<T: NodeRenderer> {
    id: Uuid,
    tcp_listener: TcpListener,
    state: Arc<Mutex<NodeState<T>>>,
}

impl<T: NodeRenderer + 'static> Node<T> {
    pub fn new(renderer: T, host_addr: &str, node_addr: &str) -> Result<Self> {
        let id = Uuid::new_v4();

        let tcp_listener = TcpListener::bind(node_addr)?;

        let state = Arc::new(Mutex::new(NodeState {
            id,
            host_addr: host_addr.to_owned(),
            node_addr: node_addr.to_owned(),
            tcp_client: None,
            renderer,
        }));

        Ok(Self {
            id,
            tcp_listener,
            state,
        })
    }

    fn handle_message(state: Arc<Mutex<NodeState<T>>>, host_message: HostMessage) -> Result<()> {
        match host_message.ty() {
            HostMessageType::StartRender => {
                log::info!("Node started Render!");

                if let Ok(mut state) = state.lock() {
                    let message = NodeMessage::new(NodeMessageType::RenderStarted, state.id);
                    state.tcp_client_write_all(bytemuck::bytes_of(&message));

                    let mut pixels = state.renderer.render(
                        host_message.width,
                        host_message.height,
                        host_message.scissor,
                    );

                    let message = NodeMessage::new(NodeMessageType::RenderFinished, state.id);
                    let mut message_bytes = bytemuck::bytes_of(&message).to_vec();
                    message_bytes.append(&mut pixels);
                    state.tcp_client_write_all(&message_bytes);
                }
            }
            HostMessageType::Ping => {
                log::info!("Node got Pinged!");

                // Notify host of this nodes existance
                if let Ok(mut state) = state.lock() {
                    let message = NodeMessage::new(NodeMessageType::Connect, state.id);
                    state.tcp_client_write_all(bytemuck::bytes_of(&message));
                }
            }
        }

        Ok(())
    }

    pub fn run(self) {
        log::info!("Node started running!");

        for stream in self.tcp_listener.incoming() {
            log::info!("Node got a message!");

            match stream {
                Ok(mut stream) => {
                    let mut buf = vec![0u8; std::mem::size_of::<HostMessage>()];
                    if let Ok(_len) = stream.read(buf.as_mut()) {
                        let host_message = *bytemuck::from_bytes::<HostMessage>(buf.as_ref());
                        let state = self.state.clone();

                        thread::spawn(move || Self::handle_message(state, host_message).unwrap());
                    }
                }
                Err(_) => {
                    panic!("Node listener panicked!");
                    return;
                }
            }
        }
    }
}

impl<T: NodeRenderer> Drop for Node<T> {
    fn drop(&mut self) {
        let message = NodeMessage::new(NodeMessageType::Disconnect, self.id);

        log::info!("DROP");
        self.state
            .lock()
            .unwrap()
            .tcp_client_write_all(bytemuck::bytes_of(&message));
    }
}
