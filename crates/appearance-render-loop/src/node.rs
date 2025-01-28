use std::io::{Read, Write};
use std::net::TcpStream;

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
    host_addr: String,
    tcp_stream: Option<TcpStream>,
    expected_package: ExpectedPackage,

    renderer: T,
}

impl<T: NodeRenderer + 'static> Node<T> {
    pub fn new(renderer: T, host_addr: &str) -> Result<Self> {
        Ok(Self {
            host_addr: host_addr.to_owned(),
            tcp_stream: None,
            expected_package: ExpectedPackage::Message,
            renderer,
        })
    }

    fn handle_message(&mut self, host_message: HostMessage) -> Result<()> {
        match host_message.ty() {
            HostMessageType::StartRender => {
                if let Some(tcp_stream) = &mut self.tcp_stream {
                    let pixels = self.renderer.render(
                        host_message.width,
                        host_message.height,
                        host_message.assigned_rows,
                    );

                    tcp_stream.write_all(pixels)?;
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
    }

    pub fn run(mut self) {
        let mut buf = vec![0u8; 1];

        loop {
            // Try to connect to host if not connected yet
            if self.tcp_stream.is_none() {
                if let Ok(tcp_stream) = TcpStream::connect(&self.host_addr) {
                    log::info!("Connected");
                    self.tcp_stream = Some(tcp_stream);
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

                            log::info!("Received message: {:?}", host_message);
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
