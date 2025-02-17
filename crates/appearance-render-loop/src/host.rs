use anyhow::Result;
use appearance_world::visible_world_action::VisibleWorldAction;
use core::{
    net::SocketAddr,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
use crossbeam::channel::Receiver;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use unreliable::{Socket, SocketEvent};

/// Size of each rendered block is a multiple of 8x8, as this is the minimum size jpeg is able to compress. This must also be a multiple of `PATH_TRACER_RAY_PACKET_SIZE`, which is 16.
pub const RENDER_BLOCK_SIZE: u32 = 64;
pub const BYTES_PER_PIXEL: usize = 4;
pub const NODE_BYTES_PER_PIXEL: usize = 3;
pub const NODE_PIXEL_FORMAT: turbojpeg::PixelFormat = turbojpeg::PixelFormat::RGB;

pub struct RenderPartialFinishedData {
    pub row: u32,
    pub column_block: u32,
    pub frame_idx: u32,
    pub compressed_pixel_bytes: Vec<u8>,
}

pub struct RenderFinishedData {
    pub frame_idx: u32,
}

pub enum NodeToHostMessage {
    RenderPartialFinished(RenderPartialFinishedData),
}

impl NodeToHostMessage {
    pub fn to_bytes(self) -> Vec<u8> {
        match self {
            NodeToHostMessage::RenderPartialFinished(mut data) => {
                let mut bytes = bytemuck::bytes_of(&0u32).to_vec();
                bytes.append(&mut bytemuck::bytes_of(&data.row).to_vec());
                bytes.append(&mut bytemuck::bytes_of(&data.column_block).to_vec());
                bytes.append(&mut bytemuck::bytes_of(&data.frame_idx).to_vec());
                bytes.append(&mut data.compressed_pixel_bytes);

                let padded_size = bytes.len().div_ceil(4) * 4;
                bytes.resize(padded_size, 0u8);

                bytes
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 4 {
            return Err(anyhow::Error::msg(
                "Failed to convert bytes to node-to-host message. (Empty bytes)",
            ));
        }

        let ty = *bytemuck::from_bytes::<u32>(&bytes[0..4]);
        match ty {
            0 => {
                let row = *bytemuck::from_bytes::<u32>(&bytes[4..8]);
                let column_block = *bytemuck::from_bytes::<u32>(&bytes[8..12]);
                let frame_idx = *bytemuck::from_bytes::<u32>(&bytes[12..16]);
                let compressed_pixel_bytes = bytes[16..bytes.len()].to_vec();
                Ok(Self::RenderPartialFinished(RenderPartialFinishedData {
                    row,
                    column_block,
                    frame_idx,
                    compressed_pixel_bytes,
                }))
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
                bytes.append(&mut bytemuck::bytes_of(&(data.must_sync as u32)).to_vec());
                bytes.append(&mut data.data);
                bytes
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 4 {
            return Err(anyhow::Error::msg(
                "Failed to convert bytes to host-to-node message. (Empty bytes)",
            ));
        }

        let ty = *bytemuck::from_bytes::<u32>(&bytes[0..4]);
        match ty {
            0 => Ok(Self::StartRender(*bytemuck::from_bytes::<StartRenderData>(
                &bytes[4..bytes.len()],
            ))),
            1 => {
                let ty = *bytemuck::from_bytes::<u32>(&bytes[4..8]);
                let must_sync = (*bytemuck::from_bytes::<u32>(&bytes[8..12])) != 0;
                let data = bytes[12..bytes.len()].to_vec();
                Ok(Self::VisibleWorldAction(VisibleWorldAction {
                    ty,
                    data,
                    must_sync,
                }))
            }
            _ => Err(anyhow::Error::msg(
                "Failed to convert bytes to host-to-node message.",
            )),
        }
    }
}

pub const BUFFERED_PIXEL_COUNT: usize = 2;

struct BufferedPixelData {
    width: u32,
    height: u32,
    pixels: [Mutex<Vec<u8>>; BUFFERED_PIXEL_COUNT],
    duplicate_map: [Mutex<HashMap<u32, bool>>; BUFFERED_PIXEL_COUNT],
    frame_idx: AtomicU32,
    received_packet_count: [AtomicU32; BUFFERED_PIXEL_COUNT],
}

impl BufferedPixelData {
    fn new(width: u32, height: u32) -> Self {
        let pixel_bytes = (width * height) as usize * BYTES_PER_PIXEL;

        let pixels = std::array::from_fn(|_| Mutex::new(vec![0; pixel_bytes]));
        let duplicate_map = std::array::from_fn(|_| Mutex::new(HashMap::new()));
        let received_packet_count = std::array::from_fn(|_| AtomicU32::new(0));

        Self {
            width,
            height,
            pixels,
            duplicate_map,
            frame_idx: AtomicU32::new(0),
            received_packet_count,
        }
    }

    fn read_pixels_idx(&self) -> usize {
        (self.frame_idx.load(Ordering::SeqCst) + 1) as usize % BUFFERED_PIXEL_COUNT
    }

    fn write_render_finished_pixels(
        &self,
        render_pixels: &[u8],
        render_partial_finished_data: RenderPartialFinishedData,
    ) {
        let idx = render_partial_finished_data.frame_idx as usize % BUFFERED_PIXEL_COUNT;

        if let Ok(mut duplicate_map) = self.duplicate_map[idx].lock() {
            assert!(render_partial_finished_data.column_block < u16::MAX as u32);
            assert!(render_partial_finished_data.row < u16::MAX as u32);

            let key = render_partial_finished_data.column_block
                | (render_partial_finished_data.row << 16);
            if duplicate_map.insert(key, true).is_some() {
                return;
            }
        }

        if let Ok(mut pixels) = self.pixels[idx].lock() {
            for local_y in 0..RENDER_BLOCK_SIZE {
                for local_x in 0..RENDER_BLOCK_SIZE {
                    let x =
                        local_x + (render_partial_finished_data.column_block * RENDER_BLOCK_SIZE);
                    let y = local_y + render_partial_finished_data.row;

                    let id = (y * self.width + x) as usize;
                    let local_id = (local_y * RENDER_BLOCK_SIZE + local_x) as usize;

                    for i in 0..NODE_BYTES_PER_PIXEL {
                        pixels[id * BYTES_PER_PIXEL + i] =
                            render_pixels[local_id * NODE_BYTES_PER_PIXEL + i];
                    }
                    pixels[id * BYTES_PER_PIXEL + BYTES_PER_PIXEL - 1] = 255;
                }
            }
        }

        self.received_packet_count[idx].fetch_add(1, Ordering::SeqCst);
    }

    fn set_pixels_pink(&self) {
        for pixels in &self.pixels {
            if let Ok(mut pixels) = pixels.lock() {
                for x in 0..self.width {
                    for y in 0..self.height {
                        pixels[(y * self.width + x) as usize * BYTES_PER_PIXEL] = 255;
                        pixels[(y * self.width + x) as usize * BYTES_PER_PIXEL + 1] = 0;
                        pixels[(y * self.width + x) as usize * BYTES_PER_PIXEL + 2] = 255;
                        pixels[(y * self.width + x) as usize * BYTES_PER_PIXEL + 3] = 255;
                    }
                }
            }
        }
    }

    fn read_pixels<F: Fn(&[u8])>(&self, callback: F) {
        if let Ok(pixels) = self.pixels[self.read_pixels_idx()].lock() {
            callback(pixels.as_ref());
        }
    }

    pub fn next_frame(&self) {
        self.duplicate_map[self.read_pixels_idx()]
            .lock()
            .unwrap()
            .clear();

        let count = self.received_packet_count[self.read_pixels_idx()].swap(0, Ordering::SeqCst);
        log::debug!("received pixel packets: {}", count);

        self.frame_idx.fetch_add(1, Ordering::SeqCst);
    }
}

pub struct Host {
    connected_nodes: Arc<Mutex<Vec<SocketAddr>>>,
    has_received_new_connections: Arc<AtomicBool>,
    socket: Socket,
    node_port: u16,

    width: u32,
    height: u32,
    pixels: Arc<BufferedPixelData>,
    frame_idx: u32,
}

impl Host {
    pub fn new(host_port: u16, node_port: u16, width: u32, height: u32) -> Result<Self> {
        let connected_nodes = Arc::new(Mutex::new(Vec::new()));
        let has_received_new_connections = Arc::new(AtomicBool::new(false));
        let pixels = Arc::new(BufferedPixelData::new(width, height));

        let mut socket = Socket::new(None, host_port)?;

        let receive_events_event_receiver = socket.event_receiver().clone();
        let recieve_events_connected_nodes = connected_nodes.clone();
        let recieve_events_has_received_new_connections = has_received_new_connections.clone();
        let recieve_events_pixels = pixels.clone();
        let recieve_events_width = width;
        thread::spawn(move || {
            Self::receive_events(
                receive_events_event_receiver,
                recieve_events_connected_nodes,
                recieve_events_has_received_new_connections,
                recieve_events_pixels,
                recieve_events_width,
            )
        });

        Ok(Self {
            connected_nodes,
            has_received_new_connections,
            socket,
            node_port,

            width,
            height,
            pixels,
            frame_idx: 0,
        })
    }

    fn receive_events(
        event_receiver: Receiver<SocketEvent>,
        connected_nodes: Arc<Mutex<Vec<SocketAddr>>>,
        has_received_new_connections: Arc<AtomicBool>,
        pixels: Arc<BufferedPixelData>,
        width: u32,
    ) {
        loop {
            if let Ok(socket_event) = event_receiver.recv() {
                match socket_event {
                    SocketEvent::Packet(packet, delay) => {
                        if delay > 1 {
                            println!("REJECT!");
                            continue;
                        }

                        if !packet.is_barrier() {
                            if let Ok(message) = NodeToHostMessage::from_bytes(packet.payload()) {
                                match message {
                                    NodeToHostMessage::RenderPartialFinished(data) => {
                                        let image = turbojpeg::decompress(
                                            &data.compressed_pixel_bytes,
                                            NODE_PIXEL_FORMAT,
                                        )
                                        .unwrap();

                                        pixels.write_render_finished_pixels(&image.pixels, data);

                                        // TODO: in the future the 8x8 blocks can be memcpied, however this will require a more advanced blit pass to display correctly
                                        // let first_dst_pixel = (data.row * width) + data.row_start;

                                        // if let Ok(mut pixels) = pixels.lock() {
                                        //     let dst_ptr = &mut pixels
                                        //         [(first_dst_pixel * 4) as usize]
                                        //         as *mut u8;
                                        //     let src_ptr = &mut data.pixels[0] as *mut u8;

                                        //     std::ptr::copy_nonoverlapping(
                                        //         src_ptr,
                                        //         dst_ptr,
                                        //         data.pixels.len(),
                                        //     );
                                        // }
                                    }
                                }
                            } else {
                                log::warn!("Failed to read message from {}.", packet.addr());
                            }
                        }
                    }
                    SocketEvent::Connect(addr) => {
                        log::info!("Node connected at {:?}", addr);
                        if let Ok(mut connected_nodes) = connected_nodes.lock() {
                            connected_nodes.push(addr);
                            has_received_new_connections.store(true, Ordering::SeqCst);
                        }
                    }
                    SocketEvent::Disconnect(addr) => {
                        log::info!("Node disconnected at {:?}...", addr);
                        if let Ok(mut connected_nodes) = connected_nodes.lock() {
                            let mut node_idx = 0;
                            for (i, node) in connected_nodes.iter().enumerate() {
                                if *node == addr {
                                    node_idx = i;
                                    break;
                                }
                            }
                            connected_nodes.remove(node_idx);
                        }
                    }
                }
            }

            thread::yield_now();
        }
    }

    pub fn send_visible_world_actions(&mut self, visible_world_actions: Vec<VisibleWorldAction>) {
        let packet_sender = self.socket.packet_sender();

        for visible_world_action in visible_world_actions {
            let must_sync = visible_world_action.must_sync;

            let message = HostToNodeMessage::VisibleWorldAction(visible_world_action);
            let message_bytes = message.to_bytes();

            if let Ok(connected_nodes) = self.connected_nodes.lock() {
                for node in connected_nodes.iter() {
                    if must_sync {
                        packet_sender
                            .send_barrier(*node, message_bytes.clone())
                            .unwrap();
                    } else {
                        // Incoming connection addresses can provide a different port than the port they actively listen on
                        // This doesn't matter for tcp as it works with handshakes, but for udp it does
                        let mut addr = *node;
                        addr.set_port(self.node_port);
                        packet_sender
                            .send_unreliable(addr, message_bytes.clone())
                            .unwrap();
                    }
                }
            }
        }
    }

    /// Returns if there were any new connections since the last time this function was called
    pub fn handle_new_connections(&mut self) -> bool {
        let has_received_new_connections = self.has_received_new_connections.load(Ordering::SeqCst);
        self.has_received_new_connections
            .store(false, Ordering::SeqCst);
        has_received_new_connections
    }

    pub fn render<F: Fn(&[u8])>(&mut self, result_callback: F) {
        if let Ok(connected_nodes) = self.connected_nodes.lock() {
            // Return pink when no nodes connected, this should be a visual warning to the host
            if connected_nodes.is_empty() {
                self.pixels.set_pixels_pink();
            } else {
                let barrier = self.socket.barrier().fetch_add(1, Ordering::SeqCst) + 1;

                let num_nodes = connected_nodes.len() as u32;
                let rows_per_node = self.height / num_nodes;

                let packet_sender = self.socket.packet_sender();

                // Notify all connected nodes to start rendering their assigned part of the screen
                for (i, node) in connected_nodes.iter().enumerate() {
                    let row_start = rows_per_node * i as u32;
                    let row_end = if i as u32 == num_nodes - 1 {
                        self.height
                    } else {
                        rows_per_node * (i as u32 + 1)
                    };

                    assert!((row_end - row_start) % RENDER_BLOCK_SIZE == 0);

                    let message = HostToNodeMessage::StartRender(StartRenderData {
                        width: self.width,
                        height: self.height,
                        row_start,
                        row_end,
                        frame_idx: self.frame_idx,
                    });
                    packet_sender
                        .send_barrier(*node, message.to_bytes())
                        .unwrap();
                }

                // Wait for all nodes to finish rendering
                loop {
                    if self.socket.barrier().load(Ordering::SeqCst) == barrier + 1 {
                        break;
                    }
                    thread::yield_now();
                }
            }
        }

        self.pixels.read_pixels(result_callback);

        self.pixels.next_frame();
        self.frame_idx += 1;
    }
}
