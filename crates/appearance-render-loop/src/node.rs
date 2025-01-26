use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use std::thread;
use uuid::Uuid;

use crate::host::{HostMessage, HostMessageType};

#[derive(bytemuck::NoUninit, bytemuck::AnyBitPattern, Clone, Copy, Default, Debug)]
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
    //node_id: [u8; 16],
}

impl NodeMessage {
    fn new(ty: NodeMessageType) -> Self {
        Self {
            ty: ty as u32,
            //node_id: node_id.to_bytes_le(),
        }
    }

    pub fn ty(&self) -> NodeMessageType {
        self.ty.into()
    }

    // pub fn node_id(&self) -> Uuid {
    //     Uuid::from_bytes(self.node_id)
    // }
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
    host_addr: String,
    node_addr: String,
    tcp_stream: Option<TcpStream>,

    renderer: T,
    //id: Uuid,

    //tcp_listener: TcpListener,
    //state: Arc<Mutex<NodeState<T>>>,
}

impl<T: NodeRenderer + 'static> Node<T> {
    pub fn new(renderer: T, host_addr: &str, node_addr: &str) -> Result<Self> {
        //let id = Uuid::new_v4();

        //let tcp_stream = TcpStream::connect(node_addr)?;

        // let state = Arc::new(Mutex::new(NodeState {
        //     id,
        //     host_addr: host_addr.to_owned(),
        //     node_addr: node_addr.to_owned(),
        //     tcp_client: None,
        //     renderer,
        // }));

        Ok(Self {
            host_addr: host_addr.to_owned(),
            node_addr: node_addr.to_owned(),
            tcp_stream: None,
            renderer,
        })
    }

    fn handle_message(&mut self, host_message: HostMessage) -> Result<()> {
        match host_message.ty() {
            HostMessageType::StartRender => {
                log::info!("StartRender");

                if let Some(tcp_stream) = &mut self.tcp_stream {
                    let message = NodeMessage::new(NodeMessageType::RenderStarted);
                    tcp_stream.write_all(bytemuck::bytes_of(&message))?;

                    let mut pixels = self.renderer.render(
                        host_message.width,
                        host_message.height,
                        host_message.scissor,
                    );

                    let message = NodeMessage::new(NodeMessageType::RenderFinished);
                    let mut message_bytes = bytemuck::bytes_of(&message).to_vec();
                    message_bytes.append(&mut pixels);
                    tcp_stream.write_all(&message_bytes)?;
                }
            }
            HostMessageType::Ping => {
                log::info!("Ping");

                // Notify host of this nodes existance
                if let Some(tcp_stream) = &mut self.tcp_stream {
                    let message = NodeMessage::new(NodeMessageType::Connect);
                    tcp_stream.write_all(bytemuck::bytes_of(&message))?;
                }
            }
        }

        Ok(())
    }

    fn disconnect(&mut self) {
        log::info!("Disconnected");
        self.tcp_stream = None;
    }

    pub fn run(mut self) {
        log::info!("Started running");

        let mut buf = vec![0u8; std::mem::size_of::<HostMessage>()];

        loop {
            // Try to connect to host if not connected yet
            if self.tcp_stream.is_none() {
                if let Ok(tcp_stream) = TcpStream::connect(&self.host_addr) {
                    log::info!("Connected");
                    self.tcp_stream = Some(tcp_stream);
                }
            }

            if let Some(tcp_stream) = &mut self.tcp_stream {
                if let Ok(_len) = tcp_stream.read(buf.as_mut()) {
                    let host_message = *bytemuck::from_bytes::<HostMessage>(buf.as_ref());

                    log::info!("Received message: {:?}", host_message);
                    if self.handle_message(host_message).is_err() {
                        self.disconnect();
                    }
                } else {
                    self.disconnect();
                }
            }

            thread::yield_now();
        }

        // for stream in self.tcp_listener.incoming() {
        //     log::info!("Node got a message!");

        //     match stream {
        //         Ok(mut stream) => {
        //             let mut buf = vec![0u8; std::mem::size_of::<HostMessage>()];
        //             if let Ok(_len) = stream.read(buf.as_mut()) {
        //                 let host_message = *bytemuck::from_bytes::<HostMessage>(buf.as_ref());
        //                 let state = self.state.clone();

        //                 thread::spawn(move || Self::handle_message(state, host_message).unwrap());
        //             }
        //         }
        //         Err(_) => {
        //             panic!("Node listener panicked!");
        //             return;
        //         }
        //     }
        // }
    }
}

impl<T: NodeRenderer> Drop for Node<T> {
    fn drop(&mut self) {
        //let message = NodeMessage::new(NodeMessageType::Disconnect, self.id);

        // log::info!("DROP");
        // self.state
        //     .lock()
        //     .unwrap()
        //     .tcp_client_write_all(bytemuck::bytes_of(&message));
    }
}
