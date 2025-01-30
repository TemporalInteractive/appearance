use anyhow::Result;
use appearance_time::Timer;
use appearance_world::visible_world_action::VisibleWorldAction;
use core::{
    net::SocketAddr,
    sync::atomic::{AtomicBool, AtomicI32, Ordering},
    time::Duration,
    u32,
};
use crossbeam::channel::{Receiver, Sender};
use std::{
    sync::{Arc, Mutex},
    thread,
};

use laminar::{Packet, Socket, SocketEvent};

pub struct RenderPartialFinishedData {
    pub row: u32,
    pub row_start: u32,
    pub pixels: Vec<u8>,
}

pub struct RenderFinishedData {
    pub frame_idx: u32,
}

pub enum NodeToHostMessage {
    RenderPartialFinished(RenderPartialFinishedData),
    Connect,
    RenderFinished(RenderFinishedData),
}

impl NodeToHostMessage {
    pub fn to_bytes(self) -> Vec<u8> {
        match self {
            NodeToHostMessage::RenderPartialFinished(mut data) => {
                let mut bytes = bytemuck::bytes_of(&0u32).to_vec();
                bytes.append(&mut bytemuck::bytes_of(&data.row).to_vec());
                bytes.append(&mut bytemuck::bytes_of(&data.row_start).to_vec());
                bytes.append(&mut data.pixels);
                bytes
            }
            NodeToHostMessage::Connect => bytemuck::bytes_of(&1u32).to_vec(),
            NodeToHostMessage::RenderFinished(data) => {
                let mut bytes = bytemuck::bytes_of(&2u32).to_vec();
                bytes.append(&mut bytemuck::bytes_of(&data.frame_idx).to_vec());
                bytes
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let ty = *bytemuck::from_bytes::<u32>(&bytes[0..4]);
        match ty {
            0 => {
                let row = *bytemuck::from_bytes::<u32>(&bytes[4..8]);
                let row_start = *bytemuck::from_bytes::<u32>(&bytes[8..12]);
                let pixels = bytes[12..bytes.len()].to_vec();
                Ok(Self::RenderPartialFinished(RenderPartialFinishedData {
                    row,
                    row_start,
                    pixels,
                }))
            }
            1 => Ok(Self::Connect),
            2 => {
                let frame_idx = *bytemuck::from_bytes::<u32>(&bytes[4..8]);
                Ok(Self::RenderFinished(RenderFinishedData { frame_idx }))
            }
            _ => Err(anyhow::Error::msg(
                "Failed to convert bytes to node-to-host message.",
            )),
        }
    }
}

#[derive(bytemuck::NoUninit, bytemuck::AnyBitPattern, Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct StartRenderData {
    pub width: u32,
    pub height: u32,
    pub row_start: u32,
    pub row_end: u32,
    pub frame_idx: u32,
}

pub enum HostToNodeMessage {
    StartRender(StartRenderData),
    VisibleWorldAction(VisibleWorldAction),
}

impl HostToNodeMessage {
    pub fn to_bytes(self) -> Vec<u8> {
        match self {
            HostToNodeMessage::StartRender(data) => {
                let mut bytes = bytemuck::bytes_of(&0u32).to_vec();
                bytes.append(&mut bytemuck::bytes_of(&data).to_vec());
                bytes
            }
            HostToNodeMessage::VisibleWorldAction(mut data) => {
                let mut bytes = bytemuck::bytes_of(&1u32).to_vec();
                bytes.append(&mut bytemuck::bytes_of(&data.ty).to_vec());
                bytes.append(&mut data.data);
                bytes
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let ty = *bytemuck::from_bytes::<u32>(&bytes[0..4]);
        match ty {
            0 => Ok(Self::StartRender(*bytemuck::from_bytes::<StartRenderData>(
                &bytes[4..bytes.len()],
            ))),
            1 => {
                let ty = *bytemuck::from_bytes::<u32>(&bytes[4..8]);
                let data = bytes[8..bytes.len()].to_vec();
                Ok(Self::VisibleWorldAction(VisibleWorldAction { ty, data }))
            }
            _ => Err(anyhow::Error::msg(
                "Failed to convert bytes to host-to-node message.",
            )),
        }
    }
}

//

struct ConnectedNode {
    addr: SocketAddr,
}

impl ConnectedNode {
    fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }
}

struct Fence {
    /// How many nodes are currently finished
    finished_nodes: AtomicI32,
    /// How many nodes have to finish before completed
    nodes_to_finish: u32,
    /// The frame this fence is associated with
    frame_idx: u32,
}

impl Fence {
    fn new(nodes_to_finish: u32, frame_idx: u32) -> Self {
        Self {
            finished_nodes: AtomicI32::new(0),
            nodes_to_finish,
            frame_idx,
        }
    }

    fn is_finished(&self) -> bool {
        self.finished_nodes.load(Ordering::SeqCst) == self.nodes_to_finish as i32
    }
}

pub struct Host {
    connected_nodes: Vec<ConnectedNode>,
    has_received_new_connections: bool,
    packet_sender: Sender<Packet>,
    event_receiver: Receiver<SocketEvent>,

    frame_idx: u32,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

pub(crate) fn laminar_config() -> laminar::Config {
    laminar::Config {
        rtt_smoothing_factor: 0.1,
        max_packets_in_flight: 2000,
        max_fragments: 16,
        socket_polling_timeout: Some(Duration::from_micros(1000)),
        ..Default::default()
    }
}

impl Host {
    pub fn new(port: &str, width: u32, height: u32) -> Result<Self> {
        let connected_nodes = Vec::new();
        let has_received_new_connections = false;
        let pixels = vec![0; (width * height * 4) as usize];

        let mut socket = Socket::bind_with_config(format!("0.0.0.0:{}", port), laminar_config())?;
        let event_receiver = socket.get_event_receiver();
        let packet_sender = socket.get_packet_sender();
        thread::spawn(move || socket.start_polling());

        // #[allow(clippy::redundant_closure_call)]
        // (|connected_nodes, has_received_new_connections, pixels, width, current_fence| {
        //     thread::spawn(move || {
        //         Self::receive_events(
        //             event_receiver,
        //             connected_nodes,
        //             has_received_new_connections,
        //             pixels,
        //             width,
        //             current_fence,
        //         )
        //     });
        // })(
        //     connected_nodes.clone(),
        //     has_received_new_connections.clone(),
        //     pixels.clone(),
        //     width,
        //     current_fence.clone(),
        // );

        Ok(Self {
            connected_nodes,
            has_received_new_connections,
            packet_sender,
            event_receiver,

            frame_idx: 0,
            width,
            height,
            pixels,
        })
    }

    fn receive_events(&mut self) {
        let expected_nodes = self.connected_nodes.len() as u32;
        let mut finished_nodes = 0;

        while finished_nodes != expected_nodes || self.connected_nodes.is_empty() {
            match self.event_receiver.recv() {
                #[allow(clippy::single_match)]
                Ok(socket_event) => match socket_event {
                    SocketEvent::Packet(packet) => {
                        if let Ok(message) = NodeToHostMessage::from_bytes(packet.payload()) {
                            match message {
                                NodeToHostMessage::RenderPartialFinished(mut data) => unsafe {
                                    let first_dst_pixel = (data.row * self.width) + data.row_start;

                                    let dst_ptr =
                                        &mut self.pixels[(first_dst_pixel * 4) as usize] as *mut u8;
                                    let src_ptr = &mut data.pixels[0] as *mut u8;

                                    std::ptr::copy_nonoverlapping(
                                        src_ptr,
                                        dst_ptr,
                                        data.pixels.len(),
                                    );
                                },
                                NodeToHostMessage::RenderFinished(data) => {
                                    if self.frame_idx == data.frame_idx {
                                        finished_nodes += 1;
                                    } else {
                                        panic!();
                                    }
                                }
                                NodeToHostMessage::Connect => {
                                    let mut already_connected = false;
                                    for node in &self.connected_nodes {
                                        if node.addr == packet.addr() {
                                            already_connected = true;
                                        }
                                    }

                                    if !already_connected {
                                        log::info!("Node connected!");
                                        self.connected_nodes
                                            .push(ConnectedNode::new(packet.addr()));
                                        self.has_received_new_connections = true;
                                    }
                                }
                            }
                        } else {
                            //log::warn!("Failed to read message from {}.", packet.addr());
                        }
                    }
                    _ => {} // SocketEvent::Connect(addr) => {}
                            // SocketEvent::Timeout(addr) | SocketEvent::Disconnect(addr) => {
                            //     // log::info!("Node disconnected...");
                            //     // if let Ok(mut connected_nodes) = connected_nodes.lock() {
                            //     //     let mut node_idx = 0;
                            //     //     for (i, node) in connected_nodes.iter().enumerate() {
                            //     //         if node.addr == addr {
                            //     //             node_idx = i;
                            //     //             break;
                            //     //         }
                            //     //     }
                            //     //     connected_nodes.remove(node_idx);
                            //     // }
                            // }
                },
                Err(e) => {
                    //log::warn!("Failed to receive event: {:?}", e);
                }
            }
        }
    }

    pub fn send_visible_world_actions(&mut self, visible_world_actions: Vec<VisibleWorldAction>) {
        for visible_world_action in visible_world_actions {
            let message = HostToNodeMessage::VisibleWorldAction(visible_world_action);
            let message_bytes = message.to_bytes();

            for node in self.connected_nodes.iter() {
                let packet = Packet::unreliable(node.addr, message_bytes.clone());
                self.packet_sender.send(packet).unwrap();
            }
        }
    }

    /// Returns if there were any new connections since the last time this function was called
    pub fn handle_new_connections(&mut self) -> bool {
        let has_received_new_connections = self.has_received_new_connections;
        self.has_received_new_connections = false;
        has_received_new_connections
    }

    pub fn render<F: Fn(&[u8])>(&mut self, result_callback: F) {
        // Return pink when no nodes connected, this should be a visual warning to the host
        if self.connected_nodes.is_empty() {
            for x in 0..self.width {
                for y in 0..self.height {
                    self.pixels[(y * self.width + x) as usize * 4] = 255;
                    self.pixels[(y * self.width + x) as usize * 4 + 1] = 0;
                    self.pixels[(y * self.width + x) as usize * 4 + 2] = 255;
                    self.pixels[(y * self.width + x) as usize * 4 + 3] = 255;
                }
            }

            //result_callback(self.pixels.as_ref());

            //return;
        } else {
            let num_nodes = self.connected_nodes.len() as u32;
            let rows_per_node = self.height / num_nodes;

            for (i, node) in self.connected_nodes.iter().enumerate() {
                let row_start = rows_per_node * i as u32;
                let row_end = if i as u32 == num_nodes - 1 {
                    self.height
                } else {
                    rows_per_node * (i as u32 + 1)
                };

                let message = HostToNodeMessage::StartRender(StartRenderData {
                    width: self.width,
                    height: self.height,
                    row_start,
                    row_end,
                    frame_idx: self.frame_idx,
                });
                let packet = Packet::unreliable(node.addr, message.to_bytes());
                self.packet_sender.send(packet).unwrap();
            }
        }

        self.receive_events();

        // loop {
        //     if let Ok(current_fence) = self.current_fence.lock() {
        //         if current_fence.is_finished() {
        //             break;
        //         }
        //     }

        //     thread::yield_now();
        // }

        result_callback(self.pixels.as_ref());

        self.frame_idx += 1;
    }
}
