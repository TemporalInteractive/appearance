use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
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
    Ping,
}

impl From<u32> for HostMessageType {
    fn from(value: u32) -> Self {
        match value {
            0 => HostMessageType::StartRender,
            1 => HostMessageType::Ping,
            _ => panic!("{} cannot be converted into a HostMessageType", value),
        }
    }
}

#[derive(bytemuck::NoUninit, bytemuck::AnyBitPattern, Clone, Copy, Default)]
#[repr(C)]
pub struct HostMessage {
    ty: u32,
    //node_id: [u8; 16],
    pub width: u32,
    pub height: u32,
    pub scissor: NodeScissor,
}

impl HostMessage {
    pub fn ty(&self) -> HostMessageType {
        self.ty.into()
    }

    // pub fn node_id(&self) -> Uuid {
    //     Uuid::from_bytes(self.node_id)
    // }
}

#[derive(Debug, PartialEq, Eq)]
enum NodeState {
    Rendering,
    Finished,
}

pub struct Host {
    host_addr: String,
    node_addr: String,

    tcp_client: Option<TcpStream>,
    nodes: Arc<Mutex<HashMap<Uuid, NodeState>>>,

    pending_scissors: Arc<Mutex<HashMap<Uuid, NodeScissor>>>,

    width: u32,
    height: u32,
    pixels: Arc<Mutex<Vec<u8>>>,
}

impl Host {
    pub fn new(host_addr: String, node_addr: String, width: u32, height: u32) -> Result<Self> {
        let pending_scissors = Arc::new(Mutex::new(HashMap::new()));
        let pixels = Arc::new(Mutex::new(vec![0; (width * height * 4) as usize]));

        let nodes = Arc::new(Mutex::new(HashMap::new()));
        let host_addrr = host_addr.clone();
        let nodess = nodes.clone();
        let pixelss = pixels.clone();
        let pending_scissorss = pending_scissors.clone();
        thread::spawn(move || {
            Self::listen(
                host_addrr,
                width,
                height,
                pixelss,
                nodess,
                pending_scissorss,
            )
        });

        Ok(Self {
            host_addr,
            node_addr,
            tcp_client: None,
            nodes,
            pending_scissors,

            width,
            height,
            pixels,
        })
    }

    // Listen for node behaviour, nodes will keep sending connect messages each time they receive any global message from the host (like StartRender)
    fn listen(
        host_addr: String,
        width: u32,
        height: u32,
        pixels: Arc<Mutex<Vec<u8>>>,
        nodes: Arc<Mutex<HashMap<Uuid, NodeState>>>,
        pending_scissors: Arc<Mutex<HashMap<Uuid, NodeScissor>>>,
    ) -> Result<()> {
        let tcp_listener = TcpListener::bind(host_addr)?;

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
                    panic!("DAMN");
                    break;
                }
            }
        }

        Ok(())
    }

    fn tcp_client_write_all(&mut self, bytes: &[u8]) {
        if self.tcp_client.is_none() {
            if let Ok(tcp_client) = TcpStream::connect(&self.node_addr) {
                log::info!("Connected tcp client.");
                self.tcp_client = Some(tcp_client);
            }
        }

        if let Some(tcp_client) = &mut self.tcp_client {
            //tcp_client.write_all(bytes).unwrap();
            if tcp_client.write_all(bytes).is_err() {
                log::info!("Tcp client lost.");
                self.tcp_client = None;
            }
        }
    }

    pub fn render<F: Fn(&[u8])>(&mut self, result_callback: F) -> Result<()> {
        // Always ping any node out there to make sure they connect
        let mut host_messages = vec![HostMessage {
            ty: HostMessageType::Ping as u32,
            ..Default::default()
        }];

        if let Ok(mut pending_scissors) = self.pending_scissors.lock() {
            // Notify all nodes to start rendering
            if let Ok(mut nodes) = self.nodes.lock() {
                log::info!("Rendering with {} nodes...", nodes.len());
                for (node_id, node_state) in nodes.iter_mut() {
                    // TODO: cut screen based on number of nodes
                    let scissor = NodeScissor {
                        scissor_x: [0, self.width],
                        scissor_y: [0, self.height],
                    };
                    pending_scissors.insert(*node_id, scissor);

                    let message = HostMessage {
                        ty: HostMessageType::StartRender as u32,
                        //node_id: node_id.to_bytes_le(),
                        width: self.width,
                        height: self.height,
                        scissor,
                    };
                    host_messages.push(message);

                    *node_state = NodeState::Rendering;
                }
            }
        }

        for message in host_messages {
            self.tcp_client_write_all(bytemuck::bytes_of(&message));
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
