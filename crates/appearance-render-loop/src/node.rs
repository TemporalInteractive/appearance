use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};

use anyhow::Result;
use appearance_world::visible_world_action::VisibleWorldActionType;
use std::thread;

use crate::host::{HostMessage, HostMessageType};

pub trait NodeRenderer {
    // TODO: world manipulation
    fn visible_world_action(&mut self, action: &VisibleWorldActionType);

    fn render(&mut self, width: u32, height: u32, assigned_rows: [u32; 2]) -> &[u8];
}

#[derive(Debug, Clone, Copy)]
enum ExpectedPackage {
    Message,
    VisibleWorldActionData(u32),
}

pub struct Node<T: NodeRenderer> {
    host_ip: String,
    tcp_port: String,
    udp_port: String,

    tcp_stream: Option<TcpStream>,
    udp_socket: Option<UdpSocket>,
    expected_package: ExpectedPackage,

    renderer: T,
}

#[derive(bytemuck::NoUninit, bytemuck::AnyBitPattern, Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct NodePixelMessageFooter {
    row: u32,
}

impl NodePixelMessageFooter {
    const NUM_ROWS: u32 = 4;

    fn new(row: u32) -> Self {
        Self { row }
    }

    pub fn row(&self) -> u32 {
        self.row
    }

    pub fn is_valid(&self) -> bool {
        true // TODO: checksum
    }
}

impl<T: NodeRenderer + 'static> Node<T> {
    pub fn new(renderer: T, host_ip: &str, tcp_port: &str, udp_port: &str) -> Result<Self> {
        Ok(Self {
            host_ip: host_ip.to_owned(),
            tcp_port: tcp_port.to_owned(),
            udp_port: udp_port.to_owned(),
            tcp_stream: None,
            udp_socket: None,
            expected_package: ExpectedPackage::Message,
            renderer,
        })
    }

    fn handle_message(&mut self, host_message: HostMessage) -> Result<()> {
        match host_message.ty() {
            HostMessageType::StartRender => {
                if let Some(udp_socket) = &mut self.udp_socket {
                    self.tcp_stream
                        .as_ref()
                        .unwrap()
                        .write_all(bytemuck::bytes_of(&100u32))?;

                    let pixels = self.renderer.render(
                        host_message.width,
                        host_message.height,
                        host_message.assigned_rows,
                    );

                    for local_row in 0..(host_message.assigned_rows[1]
                        - host_message.assigned_rows[0])
                        / NodePixelMessageFooter::NUM_ROWS
                    {
                        let local_row = local_row * 20;
                        let row = local_row + host_message.assigned_rows[0];
                        let footer = NodePixelMessageFooter::new(row);

                        let pixel_start = (local_row * host_message.width * 4) as usize;
                        let pixel_end = ((local_row + NodePixelMessageFooter::NUM_ROWS)
                            * host_message.width
                            * 4) as usize;
                        let mut pixel_row = pixels[pixel_start..pixel_end].to_vec();

                        pixel_row.append(&mut bytemuck::bytes_of(&footer).to_vec());

                        udp_socket
                            .send_to(&pixel_row, format!("{}:{}", self.host_ip, self.udp_port))?;
                    }
                }
            }
            HostMessageType::VisibleWorldAction => {
                self.expected_package =
                    ExpectedPackage::VisibleWorldActionData(host_message.visible_world_action_ty)
            }
        }

        Ok(())
    }

    fn disconnect(&mut self) {
        log::info!("Disconnected");
        self.tcp_stream = None;
        self.udp_socket = None;
    }

    pub fn run(mut self) {
        let mut buf = vec![0u8; 1];

        loop {
            // Try to connect to host if not connected yet
            if self.tcp_stream.is_none() {
                if let Ok(tcp_stream) =
                    TcpStream::connect(format!("{}:{}", self.host_ip, self.tcp_port))
                {
                    let udp_socket = UdpSocket::bind("0.0.0.0:0").unwrap();
                    //if let Ok(udp_socket) = UdpSocket::bind(&self.host_addr) {
                    log::info!("Connected");
                    self.tcp_stream = Some(tcp_stream);
                    self.udp_socket = Some(udp_socket);
                    //}
                }
            }

            if let Some(tcp_stream) = &mut self.tcp_stream {
                match self.expected_package {
                    ExpectedPackage::Message => {
                        buf.resize(std::mem::size_of::<HostMessage>(), 0);
                    }
                    ExpectedPackage::VisibleWorldActionData(visible_world_action_type) => {
                        buf.resize(
                            VisibleWorldActionType::data_size_from_ty(visible_world_action_type),
                            0,
                        );
                    }
                }

                if let Ok(_len) = tcp_stream.read(buf.as_mut()) {
                    match self.expected_package {
                        ExpectedPackage::Message => {
                            let host_message = *bytemuck::from_bytes::<HostMessage>(buf.as_ref());

                            if self.handle_message(host_message).is_err() {
                                self.disconnect();
                            }
                        }
                        ExpectedPackage::VisibleWorldActionData(visible_world_action_type) => {
                            let visible_world_action = VisibleWorldActionType::from_ty_and_bytes(
                                visible_world_action_type,
                                buf.as_ref(),
                            );

                            self.renderer.visible_world_action(&visible_world_action);

                            self.expected_package = ExpectedPackage::Message;
                        }
                    }
                } else {
                    self.disconnect();
                }
            }

            thread::yield_now();
        }
    }
}
