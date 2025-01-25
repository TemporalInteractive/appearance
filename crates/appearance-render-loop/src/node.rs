use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use std::thread;
use uuid::Uuid;

use crate::host::{HostMessage, HostMessageType};

#[derive(bytemuck::NoUninit, bytemuck::AnyBitPattern, Clone, Copy)]
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
    tcp_client: TcpStream,
    renderer: T,
}

pub struct Node<T: NodeRenderer> {
    id: Uuid,
    tcp_listener: TcpListener,
    state: Arc<Mutex<NodeState<T>>>,
}

impl<T: NodeRenderer + 'static> Node<T> {
    pub fn new(renderer: T, addr: &str) -> Result<Self> {
        let id = Uuid::new_v4();

        let mut tcp_client = TcpStream::connect(addr)?;
        let tcp_listener = TcpListener::bind(addr)?;

        let message = NodeMessage::new(NodeMessageType::Connect, id);
        tcp_client.write_all(bytemuck::bytes_of(&message))?;

        let state = Arc::new(Mutex::new(NodeState {
            id,
            tcp_client,
            renderer,
        }));

        Ok(Self {
            id,
            tcp_listener,
            state,
        })
    }

    fn handle_message(state: Arc<Mutex<NodeState<T>>>, host_message: HostMessage) -> Result<()> {
        match host_message.ty.into() {
            HostMessageType::StartRender => {
                if let Ok(mut state) = state.lock() {
                    let message = NodeMessage::new(NodeMessageType::RenderStarted, state.id);
                    state.tcp_client.write_all(bytemuck::bytes_of(&message))?;

                    let mut pixels = state.renderer.render(
                        host_message.width,
                        host_message.height,
                        host_message.scissor,
                    );

                    let message = NodeMessage::new(NodeMessageType::RenderFinished, state.id);
                    let mut message_bytes = bytemuck::bytes_of(&message).to_vec();
                    message_bytes.append(&mut pixels);
                    state.tcp_client.write_all(&message_bytes)?;
                }
            }
        }

        Ok(())
    }

    pub fn run(self) {
        for stream in self.tcp_listener.incoming() {
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
                    return;
                }
            }

            // Notify host of this nodes existance each time a message is received
            if let Ok(mut state) = self.state.lock() {
                let message = NodeMessage::new(NodeMessageType::Connect, self.id);
                let _ = state.tcp_client.write_all(bytemuck::bytes_of(&message));
            }
        }
    }
}

impl<T: NodeRenderer> Drop for Node<T> {
    fn drop(&mut self) {
        let message = NodeMessage::new(NodeMessageType::Disconnect, self.id);

        let _ = self
            .state
            .lock()
            .unwrap()
            .tcp_client
            .write_all(bytemuck::bytes_of(&message));
    }
}
