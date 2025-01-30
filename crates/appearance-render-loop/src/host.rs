use anyhow::Result;
use appearance_world::visible_world_action::VisibleWorldAction;
use core::{
    net::SocketAddr,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use crossbeam::channel::{Receiver, Sender};
use std::{
    sync::{Arc, Mutex},
    thread,
};

use laminar::{Packet, Socket, SocketEvent};

pub struct RenderFinishedData {
    pub row: u32,
    pub row_start: u32,
    pub pixels: Vec<u8>,
}

pub enum NodeToHostMessage {
    RenderFinished(RenderFinishedData),
    Connect,
}

impl NodeToHostMessage {
    pub fn to_bytes(self) -> Vec<u8> {
        match self {
            NodeToHostMessage::RenderFinished(mut data) => {
                let mut bytes = bytemuck::bytes_of(&0u32).to_vec();
                bytes.append(&mut bytemuck::bytes_of(&data.row).to_vec());
                bytes.append(&mut bytemuck::bytes_of(&data.row_start).to_vec());
                bytes.append(&mut data.pixels);
                bytes
            }
            NodeToHostMessage::Connect => bytemuck::bytes_of(&1u32).to_vec(),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let ty = *bytemuck::from_bytes::<u32>(&bytes[0..4]);
        match ty {
            0 => {
                let row = *bytemuck::from_bytes::<u32>(&bytes[4..8]);
                let row_start = *bytemuck::from_bytes::<u32>(&bytes[8..12]);
                let pixels = bytes[12..bytes.len()].to_vec();
                Ok(Self::RenderFinished(RenderFinishedData {
                    row,
                    row_start,
                    pixels,
                }))
            }
            1 => Ok(Self::Connect),
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

pub struct Host {
    connected_nodes: Arc<Mutex<Vec<ConnectedNode>>>,
    has_received_new_connections: Arc<AtomicBool>,
    packet_sender: Sender<Packet>,

    width: u32,
    height: u32,
    pixels: Arc<Mutex<Vec<u8>>>,
}

impl Host {
    pub fn new(port: &str, width: u32, height: u32) -> Result<Self> {
        let connected_nodes = Arc::new(Mutex::new(Vec::new()));
        let has_received_new_connections = Arc::new(AtomicBool::new(false));
        let pixels = Arc::new(Mutex::new(vec![0; (width * height * 4) as usize]));

        let mut socket = Socket::bind(format!("0.0.0.0:{}", port))?;
        let event_receiver = socket.get_event_receiver();
        let packet_sender = socket.get_packet_sender();
        thread::spawn(move || socket.start_polling());

        #[allow(clippy::redundant_closure_call)]
        (|connected_nodes, has_received_new_connections, pixels, width| {
            thread::spawn(move || {
                Self::receive_events(
                    event_receiver,
                    connected_nodes,
                    has_received_new_connections,
                    pixels,
                    width,
                )
            });
        })(
            connected_nodes.clone(),
            has_received_new_connections.clone(),
            pixels.clone(),
            width,
        );

        Ok(Self {
            connected_nodes,
            has_received_new_connections,
            packet_sender,

            width,
            height,
            pixels,
        })
    }

    fn receive_events(
        event_receiver: Receiver<SocketEvent>,
        connected_nodes: Arc<Mutex<Vec<ConnectedNode>>>,
        has_received_new_connections: Arc<AtomicBool>,
        pixels: Arc<Mutex<Vec<u8>>>,
        width: u32,
    ) {
        loop {
            match event_receiver.recv() {
                #[allow(clippy::single_match)]
                Ok(socket_event) => match socket_event {
                    SocketEvent::Packet(packet) => {
                        if let Ok(message) = NodeToHostMessage::from_bytes(packet.payload()) {
                            match message {
                                NodeToHostMessage::RenderFinished(mut data) => {
                                    if let Ok(mut pixels) = pixels.lock() {
                                        unsafe {
                                            let first_dst_pixel =
                                                (data.row * width) + data.row_start;

                                            let dst_ptr = &mut pixels
                                                [(first_dst_pixel * 4) as usize]
                                                as *mut u8;
                                            let src_ptr = &mut data.pixels[0] as *mut u8;

                                            std::ptr::copy_nonoverlapping(
                                                src_ptr,
                                                dst_ptr,
                                                data.pixels.len(),
                                            );
                                        }
                                    }
                                }
                                NodeToHostMessage::Connect => {
                                    if let Ok(mut connected_nodes) = connected_nodes.lock() {
                                        let mut already_connected = false;
                                        for node in connected_nodes.iter() {
                                            if node.addr == packet.addr() {
                                                already_connected = true;
                                            }
                                        }

                                        if !already_connected {
                                            log::info!("Node connected!");
                                            connected_nodes.push(ConnectedNode::new(packet.addr()));
                                            has_received_new_connections
                                                .store(true, Ordering::Relaxed);
                                        }
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
        if let Ok(connected_nodes) = self.connected_nodes.lock() {
            for visible_world_action in visible_world_actions {
                let message = HostToNodeMessage::VisibleWorldAction(visible_world_action);
                let message_bytes = message.to_bytes();

                for node in connected_nodes.iter() {
                    let packet = Packet::reliable_unordered(node.addr, message_bytes.clone());
                    self.packet_sender.send(packet).unwrap();
                }
            }
        }
    }

    /// Returns if there were any new connections since the last time this function was called
    pub fn handle_new_connections(&self) -> bool {
        let has_received_new_connections =
            self.has_received_new_connections.load(Ordering::Relaxed);
        self.has_received_new_connections
            .store(false, Ordering::Relaxed);
        has_received_new_connections
    }

    pub fn render<F: Fn(&[u8])>(&mut self, result_callback: F) {
        if let Ok(connected_nodes) = self.connected_nodes.lock() {
            // Return pink when no nodes connected, this should be a visual warning to the host
            if connected_nodes.is_empty() {
                if let Ok(mut pixels) = self.pixels.lock() {
                    for x in 0..self.width {
                        for y in 0..self.height {
                            pixels[(y * self.width + x) as usize * 4] = 255;
                            pixels[(y * self.width + x) as usize * 4 + 1] = 0;
                            pixels[(y * self.width + x) as usize * 4 + 2] = 255;
                            pixels[(y * self.width + x) as usize * 4 + 3] = 255;
                        }
                    }

                    result_callback(pixels.as_ref());
                }

                return;
            }

            let num_nodes = connected_nodes.len() as u32;
            let rows_per_node = self.height / num_nodes;

            for (i, node) in connected_nodes.iter().enumerate() {
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
                });
                let packet = Packet::reliable_unordered(node.addr, message.to_bytes());
                self.packet_sender.send(packet).unwrap();
            }
        }

        //std::thread::sleep(Duration::from_millis(50));

        if let Ok(pixels) = self.pixels.lock() {
            result_callback(pixels.as_ref());
        }
    }
}
