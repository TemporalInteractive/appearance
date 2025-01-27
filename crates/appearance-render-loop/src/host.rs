use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

use anyhow::Result;

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

#[derive(bytemuck::NoUninit, bytemuck::AnyBitPattern, Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct HostMessage {
    ty: u32,
    pub width: u32,
    pub height: u32,
    pub assigned_rows: [u32; 2],
}

impl HostMessage {
    pub fn ty(&self) -> HostMessageType {
        self.ty.into()
    }
}

#[derive(Debug, PartialEq, Eq)]
enum NodeState {
    Rendering,
    Finished,
}

struct ConnectedNode {
    tcp_stream: TcpStream,
    state: NodeState,
    pending_rows: Option<[u32; 2]>,
}

impl ConnectedNode {
    fn new(tcp_stream: TcpStream) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            tcp_stream,
            state: NodeState::Finished,
            pending_rows: None,
        }))
    }
}

pub struct Host {
    nodes: Arc<Mutex<Vec<Arc<Mutex<ConnectedNode>>>>>,

    width: u32,
    height: u32,
    pixels: Arc<Mutex<Vec<u8>>>,
}

impl Host {
    pub fn new(host_addr: String, width: u32, height: u32) -> Result<Self> {
        let pixels = Arc::new(Mutex::new(vec![0; (width * height * 4) as usize]));
        let nodes = Arc::new(Mutex::new(Vec::new()));

        #[allow(clippy::redundant_closure_call)] // TODO: ?
        (|host_addr, nodes| {
            thread::spawn(move || Self::listen(host_addr, nodes));
        })(host_addr.clone(), nodes.clone());

        Ok(Self {
            nodes,

            width,
            height,
            pixels,
        })
    }

    fn handle_node_result(
        width: u32,
        height: u32,
        pixels: Arc<Mutex<Vec<u8>>>,
        node: Arc<Mutex<ConnectedNode>>,
    ) {
        let mut buffered_pixels = vec![0u8; (4 * width * height) as usize];

        if let Ok(mut node) = node.lock() {
            if let Ok(len) = node.tcp_stream.read(buffered_pixels.as_mut()) {
                node.state = NodeState::Finished;

                if let Ok(mut pixels) = pixels.lock() {
                    if let Some(pending_rows) = &node.pending_rows {
                        let start_row = pending_rows[0];
                        let end_row = pending_rows[1];

                        unsafe {
                            let dst_ptr = &mut pixels[(start_row * width * 4) as usize] as *mut u8;
                            let src_ptr =
                                &mut buffered_pixels[(start_row * width * 4) as usize] as *mut u8;
                            std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, len);
                        }
                    }
                }

                // Place rendered pixels in the correct place
                // if let Some(scissor) = &node.pending_scissors {
                //     let num_pixels = (scissor.scissor_x[1] - scissor.scissor_x[0])
                //         * (scissor.scissor_y[1] - scissor.scissor_y[0]);
                //     let node_pixels = &buffered_pixels[0..(num_pixels * 4) as usize];

                //     if let Ok(mut pixels) = pixels.lock() {
                //         for x in scissor.scissor_x[0]..scissor.scissor_x[1] {
                //             for y in scissor.scissor_y[0]..scissor.scissor_y[1] {
                //                 let pixel_id = (y * width + x) as usize;
                //                 let node_pixel_id = ((y - scissor.scissor_y[0])
                //                     * (scissor.scissor_x[1] - scissor.scissor_x[0])
                //                     + (x - scissor.scissor_x[0]))
                //                     as usize;

                //                 for i in 0..4 {
                //                     let node_pixel_channel = node_pixels[node_pixel_id * 4 + i];
                //                     pixels[pixel_id * 4 + i] = node_pixel_channel;
                //                 }
                //             }
                //         }
                //     }
                // }
            }
        }
    }

    fn listen(host_addr: String, nodes: Arc<Mutex<Vec<Arc<Mutex<ConnectedNode>>>>>) -> Result<()> {
        let tcp_listener = TcpListener::bind(host_addr)?;

        for stream in tcp_listener.incoming() {
            match stream {
                Ok(stream) => {
                    if let Ok(mut nodes) = nodes.lock() {
                        log::info!("New render node connected!");
                        nodes.push(ConnectedNode::new(stream));
                    }
                }
                Err(_) => {
                    log::warn!("Failed to handle new node connection.");
                }
            }
        }

        Ok(())
    }

    pub fn render<F: Fn(&[u8])>(&mut self, result_callback: F) -> Result<()> {
        // Notify all nodes to start rendering
        if let Ok(mut nodes) = self.nodes.lock() {
            for node in nodes.iter_mut() {
                if let Ok(mut node) = node.lock() {
                    // TODO: cut screen based on number of nodes
                    let assigned_rows = [0, self.height];
                    node.pending_rows = Some(assigned_rows);

                    let message = HostMessage {
                        ty: HostMessageType::StartRender as u32,
                        width: self.width,
                        height: self.height,
                        assigned_rows,
                    };
                    node.tcp_stream
                        .write_all(bytemuck::bytes_of(&message))
                        .unwrap(); // TODO: always carefully remove node if write fails (node probably disconnected)

                    node.state = NodeState::Rendering;
                }
            }

            let mut join_handles = vec![];

            for node in nodes.iter() {
                let cloned_node = node.clone();
                let cloned_pixels = self.pixels.clone();
                let width = self.width;
                let height = self.height;

                join_handles.push(
                    thread::Builder::new()
                        .spawn(move || {
                            Self::handle_node_result(width, height, cloned_pixels, cloned_node)
                        })
                        .unwrap(),
                );
            }

            for join_handle in join_handles {
                join_handle.join().unwrap();
            }
        }

        if let Ok(pixels) = self.pixels.lock() {
            result_callback(pixels.as_ref());
        }

        Ok(())
    }
}
