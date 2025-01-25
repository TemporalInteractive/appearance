use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{Arc, Mutex},
    thread,
};

use anyhow::Result;
use uuid::Uuid;

use crate::node::{NodeMessage, NodeMessageType, NodeScissor};

#[derive(bytemuck::NoUninit, Clone, Copy)]
#[repr(u32)]
pub enum HostMessageType {
    StartRender,
}

impl From<u32> for HostMessageType {
    fn from(value: u32) -> Self {
        match value {
            0 => HostMessageType::StartRender,
            _ => panic!(),
        }
    }
}

#[derive(bytemuck::NoUninit, bytemuck::AnyBitPattern, Clone, Copy)]
#[repr(C)]
pub struct HostMessage {
    pub ty: u32,
    node_id: [u8; 16],
    pub width: u32,
    pub height: u32,
    pub scissor: NodeScissor,
}

impl HostMessage {
    pub fn node_id(&self) -> Uuid {
        Uuid::from_bytes(self.node_id)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum NodeState {
    Rendering,
    Finished,
}

pub struct Host {
    addr: String,
    tcp_client: TcpStream,
    udp_client: UdpSocket,
    nodes: Arc<Mutex<HashMap<Uuid, NodeState>>>,

    pending_scissors: Arc<Mutex<HashMap<Uuid, NodeScissor>>>,

    width: u32,
    height: u32,
    pixels: Arc<Mutex<Vec<u8>>>,
}

impl Host {
    pub fn new(addr: String, width: u32, height: u32) -> Result<Self> {
        let tcp_client = TcpStream::connect(&addr)?;

        let udp_client = UdpSocket::bind(&addr)?;

        let pending_scissors = Arc::new(Mutex::new(HashMap::new()));
        let pixels = Arc::new(Mutex::new(vec![0; (width * height * 4) as usize]));

        let nodes = Arc::new(Mutex::new(HashMap::new()));
        let addrr = addr.clone();
        let nodess = nodes.clone();
        let pixelss = pixels.clone();
        let pending_scissorss = pending_scissors.clone();
        thread::spawn(move || {
            Self::listen(addrr, width, height, pixelss, nodess, pending_scissorss)
        });

        Ok(Self {
            addr,
            tcp_client,
            udp_client,
            nodes,
            pending_scissors,

            width,
            height,
            pixels,
        })
    }

    // Listen for node behaviour, nodes will keep sending connect messages each time they receive any global message from the host (like StartRender)
    fn listen(
        addr: String,
        width: u32,
        height: u32,
        pixels: Arc<Mutex<Vec<u8>>>,
        nodes: Arc<Mutex<HashMap<Uuid, NodeState>>>,
        pending_scissors: Arc<Mutex<HashMap<Uuid, NodeScissor>>>,
    ) -> Result<()> {
        let tcp_listener = TcpListener::bind(addr)?;

        // TODO: maybe derive this size a bit more elegantly
        let mut buf = vec![0u8; std::mem::size_of::<NodeMessage>() + (4 * width * height) as usize];

        for stream in tcp_listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    if let Ok(_len) = stream.read(buf.as_mut()) {
                        let node_message = *bytemuck::from_bytes::<NodeMessage>(
                            &buf[0..std::mem::size_of::<NodeMessage>()],
                        );

                        if let Ok(mut nodes) = nodes.lock() {
                            match node_message.ty() {
                                NodeMessageType::Connect => {
                                    log::info!("Node {} connected!", node_message.node_id());
                                    nodes.insert(node_message.node_id(), NodeState::Finished);
                                }
                                NodeMessageType::Disconnect => {
                                    log::info!("Node {} disconnected!", node_message.node_id());
                                    nodes.remove(&node_message.node_id());
                                }
                                NodeMessageType::RenderStarted => {
                                    if let Some(node_state) = nodes.get_mut(&node_message.node_id())
                                    {
                                        *node_state = NodeState::Rendering;
                                    }
                                }
                                NodeMessageType::RenderFinished => {
                                    if let Some(node_state) = nodes.get_mut(&node_message.node_id())
                                    {
                                        *node_state = NodeState::Finished;

                                        // Place rendered pixels in the correct place
                                        if let Ok(pending_scissors) = pending_scissors.lock() {
                                            let scissor = pending_scissors
                                                .get(&node_message.node_id())
                                                .unwrap();

                                            let num_pixels = (scissor.scissor_x[1]
                                                - scissor.scissor_x[0])
                                                * (scissor.scissor_y[1] - scissor.scissor_y[0]);

                                            let node_pixels =
                                                &buf[std::mem::size_of::<NodeMessage>()
                                                    ..num_pixels as usize];

                                            if let Ok(mut pixels) = pixels.lock() {
                                                for x in scissor.scissor_x[0]..scissor.scissor_x[1]
                                                {
                                                    for y in
                                                        scissor.scissor_y[0]..scissor.scissor_y[1]
                                                    {
                                                        let pixel_id = (y * width + x) as usize;
                                                        let node_pixel_id = ((y - scissor
                                                            .scissor_y[0])
                                                            * (scissor.scissor_x[1]
                                                                - scissor.scissor_x[0])
                                                            + (x - scissor.scissor_x[0]))
                                                            as usize;

                                                        for i in 0..4 {
                                                            pixels[pixel_id * 4 + i] =
                                                                node_pixels[node_pixel_id * 4 + i];
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    break;
                }
            }
        }

        Ok(())
    }

    pub fn render<F: Fn(&[u8])>(&mut self, result_callback: F) -> Result<()> {
        if let Ok(mut pending_scissors) = self.pending_scissors.lock() {
            // Notify all nodes to start rendering
            if let Ok(mut nodes) = self.nodes.lock() {
                for (node_id, node_state) in nodes.iter_mut() {
                    // TODO: cut screen based on number of nodes
                    let scissor = NodeScissor {
                        scissor_x: [0, self.width],
                        scissor_y: [0, self.height],
                    };
                    pending_scissors.insert(*node_id, scissor);

                    let message = HostMessage {
                        ty: HostMessageType::StartRender as u32,
                        node_id: node_id.to_bytes_le(),
                        width: self.width,
                        height: self.height,
                        scissor,
                    };
                    self.tcp_client.write_all(bytemuck::bytes_of(&message))?;

                    *node_state = NodeState::Rendering;
                }
            }
        }

        // Blocking while waiting for all nodes to finish rendering
        loop {
            if let Ok(mut pending_scissors) = self.pending_scissors.lock() {
                let mut finished_node_ids = vec![];

                for (node_id, _pending_scissor) in pending_scissors.iter() {
                    if let Ok(nodes) = self.nodes.lock() {
                        let node_state = nodes.get(node_id).unwrap();
                        if *node_state == NodeState::Finished {
                            finished_node_ids.push(*node_id);
                        }
                    }
                }

                for node_id in finished_node_ids {
                    pending_scissors.remove(&node_id);
                }

                if pending_scissors.is_empty() {
                    break;
                }
            }

            thread::yield_now();
        }

        if let Ok(pixels) = self.pixels.lock() {
            result_callback(pixels.as_ref());
        }

        Ok(())
    }
}
