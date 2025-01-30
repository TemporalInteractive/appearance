use core::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{Arc, Mutex},
    thread,
};

use crate::node::NodePixelMessageFooter;

use anyhow::Result;
use appearance_world::visible_world_action::VisibleWorldAction;

fn vec_remove_multiple<T>(vec: &mut Vec<T>, indices: &mut Vec<usize>) {
    indices.sort();
    indices.dedup();
    for (j, i) in indices.iter().enumerate() {
        vec.remove(i - j);
    }
}

#[derive(bytemuck::NoUninit, Clone, Copy)]
#[repr(u32)]
pub enum HostMessageType {
    StartRender,
    VisibleWorldAction,
}

impl From<u32> for HostMessageType {
    fn from(value: u32) -> Self {
        match value {
            0 => HostMessageType::StartRender,
            1 => HostMessageType::VisibleWorldAction,
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
    pub visible_world_action_ty: u32,
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
    has_received_new_connections: Arc<AtomicBool>,

    width: u32,
    height: u32,
    pixels: Arc<Mutex<Vec<u8>>>,
}

impl Host {
    pub fn new(tcp_port: String, udp_port: String, width: u32, height: u32) -> Result<Self> {
        let pixels = Arc::new(Mutex::new(vec![0; (width * height * 4) as usize]));
        let nodes = Arc::new(Mutex::new(Vec::new()));
        let has_received_new_connections = Arc::new(AtomicBool::new(false));

        #[allow(clippy::redundant_closure_call)]
        (|host_addr, nodes, has_received_new_connections| {
            thread::spawn(move || Self::listen(host_addr, nodes, has_received_new_connections));
        })(
            format!("0.0.0.0:{}", tcp_port),
            nodes.clone(),
            has_received_new_connections.clone(),
        );

        #[allow(clippy::redundant_closure_call)]
        (|udp_addr, width, height, pixels| {
            thread::spawn(move || Self::handle_node_results(udp_addr, width, height, pixels));
        })(
            format!("0.0.0.0:{}", udp_port),
            width,
            height,
            pixels.clone(),
        );

        Ok(Self {
            nodes,
            has_received_new_connections,

            width,
            height,
            pixels,
        })
    }

    // fn handle_node_result(
    //     width: u32,
    //     height: u32,
    //     pixels: Arc<Mutex<Vec<u8>>>,
    //     node: Arc<Mutex<ConnectedNode>>,
    // ) {
    //     let mut buffered_pixels = vec![0u8; (4 * width * height) as usize];

    //     if let Ok(mut node) = node.lock() {
    //         if let Ok(len) = node.tcp_stream.read(buffered_pixels.as_mut()) {
    //             // Node can send data with size of 0 when in the process of disconnecting
    //             if len != 0 {
    //                 node.state = NodeState::Finished;

    //                 if let Ok(mut pixels) = pixels.lock() {
    //                     if let Some(pending_rows) = &node.pending_rows {
    //                         let start_row = pending_rows[0];

    //                         unsafe {
    //                             let dst_ptr =
    //                                 &mut pixels[(start_row * width * 4) as usize] as *mut u8;
    //                             let src_ptr = &mut buffered_pixels[0] as *mut u8;

    //                             let num_bytes =
    //                                 ((pending_rows[1] - pending_rows[0]) * width * 4) as usize;
    //                             assert_eq!(len, num_bytes);

    //                             std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, num_bytes);
    //                         }
    //                     }
    //                 }
    //             }
    //         }
    //     }
    // }

    fn listen(
        host_addr: String,
        nodes: Arc<Mutex<Vec<Arc<Mutex<ConnectedNode>>>>>,
        has_received_new_connections: Arc<AtomicBool>,
    ) -> Result<()> {
        let tcp_listener = TcpListener::bind(host_addr)?;

        for stream in tcp_listener.incoming() {
            match stream {
                Ok(stream) => {
                    if let Ok(mut nodes) = nodes.lock() {
                        log::info!("New render node connected!");
                        nodes.push(ConnectedNode::new(stream));
                        has_received_new_connections.store(true, Ordering::Relaxed);
                    }
                }
                Err(_) => {
                    log::warn!("Failed to handle new node connection.");
                }
            }
        }

        Ok(())
    }

    fn handle_node_results(
        udp_addr: String,
        width: u32,
        height: u32,
        pixels: Arc<Mutex<Vec<u8>>>,
    ) -> Result<()> {
        let udp_socket = UdpSocket::bind(udp_addr)?;

        let footer_size = std::mem::size_of::<NodePixelMessageFooter>();
        let mut buf =
            vec![0u8; (width * 4 * NodePixelMessageFooter::NUM_ROWS) as usize + footer_size];

        loop {
            if let Ok((len, src)) = udp_socket.recv_from(&mut buf) {
                log::info!("src: {:?}", src);

                let footer_bytes = &buf[(len - footer_size)..len];
                let footer = *bytemuck::from_bytes::<NodePixelMessageFooter>(footer_bytes);

                if footer.is_valid() {
                    if let Ok(mut pixels) = pixels.lock() {
                        unsafe {
                            let dst_ptr =
                                &mut pixels[(footer.row() as u32 * width * 4) as usize] as *mut u8;
                            let src_ptr = &mut buf[0] as *mut u8;

                            let num_bytes = (width * 4 * NodePixelMessageFooter::NUM_ROWS) as usize;

                            std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, num_bytes);
                        }
                    }
                }
            } else {
                // Stop reading back pixels if there's a faulty connection to a node
                //break; ???
            }
        }
    }

    pub fn send_visible_world_actions(&mut self, visible_world_actions: &[VisibleWorldAction]) {
        if let Ok(mut nodes) = self.nodes.lock() {
            let mut disconnected_node_indices = vec![];

            for (i, node) in nodes.iter_mut().enumerate() {
                if let Ok(mut node) = node.lock() {
                    for visible_world_action in visible_world_actions {
                        // Send message type, this way the node knows how to interpret the data package
                        let message = HostMessage {
                            ty: HostMessageType::VisibleWorldAction as u32,
                            visible_world_action_ty: visible_world_action.ty,
                            ..Default::default()
                        };

                        if node
                            .tcp_stream
                            .write_all(bytemuck::bytes_of(&message))
                            .is_err()
                        {
                            disconnected_node_indices.push(i);
                            continue;
                        }

                        if node
                            .tcp_stream
                            .write_all(visible_world_action.data.as_slice())
                            .is_err()
                        {
                            disconnected_node_indices.push(i);
                        }
                    }
                }
            }

            vec_remove_multiple(&mut nodes, &mut disconnected_node_indices);
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
        // Notify all nodes to start rendering
        if let Ok(mut nodes) = self.nodes.lock() {
            // Return pink when no nodes connected, this should be a visual warning to the host
            if nodes.is_empty() {
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

            let mut disconnected_node_indices = vec![];

            let num_nodes = nodes.len() as u32;
            let rows_per_node = self.height / num_nodes;

            for (i, node) in nodes.iter_mut().enumerate() {
                if let Ok(mut node) = node.lock() {
                    let rows_start = rows_per_node * i as u32;
                    let rows_end = if i as u32 == num_nodes - 1 {
                        self.height
                    } else {
                        rows_per_node * (i as u32 + 1)
                    };

                    let assigned_rows = [rows_start, rows_end];
                    node.pending_rows = Some(assigned_rows);

                    let message = HostMessage {
                        ty: HostMessageType::StartRender as u32,
                        width: self.width,
                        height: self.height,
                        assigned_rows,
                        ..Default::default()
                    };
                    if node
                        .tcp_stream
                        .write_all(bytemuck::bytes_of(&message))
                        .is_err()
                    {
                        disconnected_node_indices.push(i);
                    }

                    node.state = NodeState::Rendering;
                }
            }

            vec_remove_multiple(&mut nodes, &mut disconnected_node_indices);

            // Wait for all nodes to receive world activity events and the render events
            // At this point the nodes are may not be done with rendering yet, but that's acceptable
            for node in nodes.iter_mut() {
                if let Ok(mut node) = node.lock() {
                    let mut buf = vec![0u8; 4];
                    if let Ok(len) = node.tcp_stream.read(buf.as_mut()) {
                        assert!(len == 4);
                    }
                }
            }

            // let mut received_rows_table = vec![false; self.height as usize];
            // let mut num_received_rows = 0;

            // let footer_size = std::mem::size_of::<NodePixelMessageFooter>();
            // let mut buf = vec![0u8; (self.width * 4) as usize + footer_size];

            // while num_received_rows < self.height {
            //     if let Ok((len, _src)) = self.udp_socket.recv_from(&mut buf) {
            //         let footer_bytes = &buf[(len - footer_size)..len];
            //         let footer = *bytemuck::from_bytes::<NodePixelMessageFooter>(footer_bytes);

            //         if footer.is_valid() {
            //             if !received_rows_table[footer.row() as usize] {
            //                 received_rows_table[footer.row() as usize] = true;

            //                 unsafe {
            //                     let dst_ptr = &mut self.pixels
            //                         [(footer.row() as u32 * self.width * 4) as usize]
            //                         as *mut u8;
            //                     let src_ptr = &mut buf[0] as *mut u8;

            //                     let num_bytes = (self.width * 4) as usize;

            //                     std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, num_bytes);
            //                 }
            //             } else {
            //                 log::warn!("Received row {} too often!", footer.row());
            //             }
            //         }

            //         num_received_rows += 1;
            //     } else {
            //         // Stop reading back pixels if there's a faulty connection to a node
            //         break;
            //     }
            // }

            // let mut join_handles = vec![];
            // for node in nodes.iter() {
            //     let cloned_node = node.clone();
            //     let cloned_pixels = self.pixels.clone();
            //     let width = self.width;
            //     let height = self.height;

            //     join_handles.push(
            //         thread::Builder::new()
            //             .spawn(move || {
            //                 Self::handle_node_result(width, height, cloned_pixels, cloned_node)
            //             })
            //             .unwrap(),
            //     );
            // }

            // for join_handle in join_handles {
            //     join_handle.join().unwrap();
            // }

            std::thread::sleep(Duration::from_millis(20));
        }

        if let Ok(pixels) = self.pixels.lock() {
            result_callback(pixels.as_ref());
        }
    }
}
