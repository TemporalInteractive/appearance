use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

use anyhow::Result;

use crate::node::NodeScissor;

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
    pub scissor: NodeScissor,
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
    pending_scissors: Option<NodeScissor>,
}

impl ConnectedNode {
    fn new(tcp_stream: TcpStream) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            tcp_stream,
            state: NodeState::Finished,
            pending_scissors: None,
        }))
    }
}

pub struct Host {
    host_addr: String,
    node_addr: String,

    nodes: Arc<Mutex<Vec<Arc<Mutex<ConnectedNode>>>>>,

    // tcp_streams: Arc<Mutex<Vec<TcpStream>>>,
    // nodes: Arc<Mutex<HashMap<Uuid, NodeState>>>,
    // pending_scissors: Arc<Mutex<HashMap<Uuid, NodeScissor>>>,
    width: u32,
    height: u32,
    pixels: Arc<Mutex<Vec<u8>>>,
}

impl Host {
    #[allow(clippy::redundant_closure_call)] // TODO: ?
    pub fn new(host_addr: String, node_addr: String, width: u32, height: u32) -> Result<Self> {
        let pixels = Arc::new(Mutex::new(vec![0; (width * height * 4) as usize]));
        let nodes = Arc::new(Mutex::new(Vec::new()));

        (|host_addr, nodes| {
            thread::spawn(move || Self::listen(host_addr, nodes));
        })(host_addr.clone(), nodes.clone());

        Ok(Self {
            host_addr,
            node_addr,
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
            if let Ok(_len) = node.tcp_stream.read(buffered_pixels.as_mut()) {
                node.state = NodeState::Finished;

                // Place rendered pixels in the correct place
                if let Some(scissor) = &node.pending_scissors {
                    let num_pixels = (scissor.scissor_x[1] - scissor.scissor_x[0])
                        * (scissor.scissor_y[1] - scissor.scissor_y[0]);
                    let node_pixels = &buffered_pixels[0..(num_pixels * 4) as usize];

                    if let Ok(mut pixels) = pixels.lock() {
                        for x in scissor.scissor_x[0]..scissor.scissor_x[1] {
                            for y in scissor.scissor_y[0]..scissor.scissor_y[1] {
                                let pixel_id = (y * width + x) as usize;
                                let node_pixel_id = ((y - scissor.scissor_y[0])
                                    * (scissor.scissor_x[1] - scissor.scissor_x[0])
                                    + (x - scissor.scissor_x[0]))
                                    as usize;

                                for i in 0..4 {
                                    let node_pixel_channel = node_pixels[node_pixel_id * 4 + i];
                                    pixels[pixel_id * 4 + i] = node_pixel_channel;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn listen(host_addr: String, nodes: Arc<Mutex<Vec<Arc<Mutex<ConnectedNode>>>>>) -> Result<()> {
        let tcp_listener = TcpListener::bind(host_addr)?;

        for stream in tcp_listener.incoming() {
            match stream {
                Ok(stream) => {
                    if let Ok(mut nodes) = nodes.lock() {
                        log::info!("Connected");
                        nodes.push(ConnectedNode::new(stream));
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

    // fn tcp_client_write_all(&mut self, bytes: &[u8]) {
    //     if self.tcp_client.is_none() {
    //         if let Ok(tcp_client) = TcpStream::connect(&self.node_addr) {
    //             log::info!("Connected tcp client.");
    //             self.tcp_client = Some(tcp_client);
    //         }
    //     }

    //     if let Some(tcp_client) = &mut self.tcp_client {
    //         //tcp_client.write_all(bytes).unwrap();
    //         if tcp_client.write_all(bytes).is_err() {
    //             log::info!("Tcp client lost.");
    //             self.tcp_client = None;
    //         }
    //     }
    // }

    pub fn render<F: Fn(&[u8])>(&mut self, result_callback: F) -> Result<()> {
        // // Always ping any node out there to make sure they connect
        // let mut host_messages = vec![HostMessage {
        //     ty: HostMessageType::Ping as u32,
        //     ..Default::default()
        // }];

        log::info!("RENDER");

        // Notify all nodes to start rendering
        if let Ok(mut nodes) = self.nodes.lock() {
            log::info!("Rendering with {} nodes...", nodes.len());

            for node in nodes.iter_mut() {
                if let Ok(mut node) = node.lock() {
                    // TODO: cut screen based on number of nodes
                    let scissor = NodeScissor {
                        scissor_x: [0, self.width],
                        scissor_y: [0, self.height],
                    };
                    node.pending_scissors = Some(scissor);

                    let message = HostMessage {
                        ty: HostMessageType::StartRender as u32,
                        //node_id: node_id.to_bytes_le(),
                        width: self.width,
                        height: self.height,
                        scissor,
                    };
                    node.tcp_stream
                        .write_all(bytemuck::bytes_of(&message))
                        .unwrap(); // TODO: always carefully remove node if write fails (node probably disconnected)

                    node.state = NodeState::Rendering;
                }
            }

            let mut join_handles = vec![];

            // let join_handle: thread::JoinHandle<_> = builder
            //     .spawn(|| {
            //         // some work here
            //     })
            //     .unwrap();
            // join_handle
            //     .join()
            //     .expect("Couldn't join on the associated thread");

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

                // (|node, pixels| {
                //     thread::spawn(move || {
                //         Self::handle_node_result(self.width, self.height, pixels, node)
                //     });
                // })(node.clone(), self.pixels.clone());
            }

            for join_handle in join_handles {
                join_handle.join().unwrap();
            }
        }

        // for message in host_messages {
        //     self.tcp_client_write_all();
        // }

        // Blocking while waiting for all nodes to finish rendering
        // loop {
        //     if let Ok(nodes) = self.nodes.lock() {
        //         let mut all_finished = true;
        //         for node in nodes.iter() {
        //             if node.state == NodeState::Rendering {
        //                 all_finished = false;
        //                 break;
        //             }
        //         }

        //         if all_finished {
        //             break;
        //         }
        //     }

        //     thread::yield_now();
        // }

        if let Ok(pixels) = self.pixels.lock() {
            result_callback(pixels.as_ref());
        }

        Ok(())
    }
}
